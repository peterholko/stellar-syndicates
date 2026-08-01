//! §hyperspace — LANES: the spline network embedded in the hyperspace overlay.
//!
//! THE MODEL, in one breath. Hyperspace is an overlay on the same coordinates as
//! normal space; a ship with its drive on is in it, anywhere, outside a gravity
//! well. Lanes are ribbons within hyperspace where the lattice strain is deeper,
//! so everything there — matter and signal alike — moves faster by the same
//! factor. Three regimes: normal `1×`, hyperspace `H×`, lane `H × LANE_MULT ×`.
//!
//! WHY THE STRAIN IS UNIFORM. Because it scales ships and signals together, the
//! `c / v` ratio is identical in every regime, so a signal can always take a
//! ship's own route and beat it by that same ratio. The information model needs
//! no special case and `c` never had to be rescaled to protect it. This is the
//! refractive-index reading: strain has one value at a point.
//!
//! WHAT LANES ARE NOT. Not currents — nothing carries you, there is no flow, no
//! drift, no upstream. Not graph edges — a lane is continuous geometry that runs
//! *near* several systems, and membership is a distance test against the drawn
//! ribbon, never a lookup. Not permission — a controlled lane is always
//! bypassable at the cost of time, fuel and exposure.
//!
//! THE FICTION GENERATES THE TOPOLOGY. The lattice crystallized like frost, and
//! frost has many nucleation points: the Wormhole Hub is the primary one (the
//! radial trunks), and every system's paired singularity is a secondary one (the
//! chords that criss-cross between neighbours). Junctions are where the fronts
//! met. Nothing here is hand-authored.

use serde::{Deserialize, Serialize};

use crate::math::Vec2;
use crate::rng::Rng;

// --- TUNABLES ----------------------------------------------------------------

/// `H` — the HYPERSPACE speed factor: how much faster a hull moves with its
/// drive on, off-lane, than it does in normal space.
///
/// This is the one dial that governs how punishing it is to travel with the
/// drive OFF. In-lane and off-lane hyperspace travel times are invariant to it
/// (both the galaxy and the speeds scale together); only normal-space travel
/// changes. 2 lets a dark fleet genuinely prowl; 10 anchors it where it stands.
/// 5 is the first playtest value: clear separation without stranding anyone.
pub const WARP_FACTOR: f64 = 5.0;

/// How much faster the HYPERSPACE drive is than warp. Relative to warp, NOT to
/// thrusters — so a lane runs at `WARP_FACTOR × LANE_MULT` times thruster speed.
pub const LANE_MULT: f64 = 10.0;

/// §hyperspace: how much the galaxy grows so that INFORMATION DELAY lands where
/// the playtest wants it.
///
/// The target is on the FASTEST route: hub → rim along a trunk should be ~20 s,
/// because that is how quickly news can possibly cross the map. Everything
/// slower follows from it — off-lane hyperspace is `LANE_MULT` times worse, and
/// normal space worse again. That spread is the whole point: the lane network is
/// the information network, and being off it is genuinely remote rather than
/// marginally inconvenient.
/// INDEPENDENT of the drive factors, deliberately. It was `WARP_FACTOR ×
/// LANE_MULT`, which tied map size to engine speed: making the drives faster
/// grew the galaxy by the same ratio and every journey took exactly as long as
/// before. Tuning one has to be able to move without the other following it.
pub const GALAXY_SCALE: f64 = 50.0;

/// §course-change: how long a drive takes to spin up or shut down, in seconds.
///
/// A ship cannot steer in warp or hyperspace — to change course it drops all the
/// way back to thrusters, turns, and re-enters. These are what that costs, and
/// they are the whole reason a course change is a decision rather than a
/// keystroke. The lane figures are larger because threading a ribbon is a harder
/// piece of navigation than simply lighting a drive in open space.
///
/// This REPLACES the old bounded-turn-rate model. Course changes used to be
/// governed by a turning circle, which made an in-lane reversal a question of
/// geometry — and the geometry lost: past the alignment gate a hull's speed fell
/// tenfold and its turning circle with it, so a Titan could come about inside a
/// ribbon it was supposedly too big to turn in.
pub const WARP_SPOOL_S: f64 = 1.5;
pub const WARP_DROP_S: f64 = 1.0;
pub const LANE_SPOOL_S: f64 = 4.0;
pub const LANE_DROP_S: f64 = 3.0;

/// §course-change: how far off its committed heading a WARP fleet may be before
/// it has to shut down and come about.
///
/// Small, because in warp nothing is steering: the fleet flies the line it was
/// aimed along when the drive caught. Anything beyond a rounding error means the
/// course it wants is not the course it is on, and the only way to fix that is
/// to drop out. A fleet riding a LANE is exempt — there the road is steering,
/// and a road bending is not the ship changing its mind.
pub const COURSE_LOCK_RAD: f64 = 0.09; // ~5°

/// Seconds to spin a drive up into `to`. Thrusters need no spin-up.
pub fn spool_seconds(to: Regime) -> f64 {
    match to {
        Regime::Thrusters => 0.0,
        Regime::Warp => WARP_SPOOL_S,
        Regime::Hyperspace => LANE_SPOOL_S,
    }
}

/// Seconds to shut `from` down to thrusters.
pub fn drop_seconds(from: Regime) -> f64 {
    match from {
        Regime::Thrusters => 0.0,
        Regime::Warp => WARP_DROP_S,
        Regime::Hyperspace => LANE_DROP_S,
    }
}

/// Turn-radius constant: `min_turn_radius = TURN_K × speed`. Constant speed is
/// preserved (this is emphatically not the flip-and-burn model §7 removed) —
/// only heading is rate-limited, so `t = d/v` still holds along the flown path
/// and `uncertainty = age × speed` stays exact.
///
/// It is load-bearing rather than flavour: at lane speed every hull's turning
/// circle is wider than the ribbon, so **coming about inside a lane is
/// physically impossible** and reversal necessarily means leaving, arcing, and
/// re-entering. The cost of that manoeuvre is `π × TURN_K` seconds of turn for
/// every hull, plus however long the lateral exit takes — which is what makes it
/// hurt more for heavy hulls.
pub const TURN_K: f64 = 5.7;

/// A system's GRAVITY WELL: inside this radius the hyperspace drive will not
/// light, so a fleet there is in normal space, slow, and committed.
///
/// One rule, and it protects everything: blockade, siege, defense platforms,
/// corvette screens, docking and the whole ground war keep working untouched,
/// and nobody hyperspaces out from under an investment. It also gives systems
/// their tactical character — they are traps as much as anchors. Matches the
/// blockade station radius, which is the established "at the system" scale.
pub const HYPERLIMIT: f64 = 900.0;

/// Minimum CENTERLINE clearance every lane keeps from every gravity well.
///
/// The ribbon may visually brush the protected circle; the road itself never
/// threads a system. Fleets leave the lane and cross the remaining gap in open
/// hyperspace before the well forces them down to thrusters.
pub const WELL_STANDOFF: f64 = HYPERLIMIT * 2.5;

/// Ribbon width as a fraction of the median neighbouring-system distance. One
/// standardized width for v1 — no minor/major/super classes until the standard
/// model is understood.
pub const LANE_WIDTH_FRAC: f64 = 0.26;

/// What fraction of its width a route keeps at its very tip. The taper thins the
/// frontier; this stops it thinning to a ribbon with no inside.
const TAPER_FLOOR_FRAC: f64 = 0.35;

/// A heading must lie within this of the local tangent to earn the lane's speed.
/// The ALIGNMENT GATE: it stops a lane being a free boost for traffic merely
/// crossing it, makes the directional bands mean something, and makes a reversal
/// cost the whole arc because the benefit drops the moment you turn off-axis.
pub const ALIGN_TOLERANCE_RAD: f64 = 0.35; // ~20°

/// Maximum heading change per step while a route is being grown. This — not the
/// curvature repair — is what actually shapes routes and makes hairpins
/// structurally impossible rather than fixed up afterwards.
const GROW_MAX_TURN_RAD: f64 = 0.61; // ~35°

/// Headroom an arc keeps over the fastest hull's turning circle when it settles
/// its anchor pull. Nothing perturbs a route after generation any more, so this
/// is no longer absorbing a later pass — it is margin against the pull landing
/// right on the bar, which would leave an arc technically flyable and horrible.
const ARC_CURVATURE_MARGIN: f64 = 1.3;

/// Step length bounds for route growth, as fractions of median system spacing.
/// Deliberately longer than one hop: a route that stepped to its nearest
/// neighbour every time would touch every system in the galaxy and the map would
/// come out uniformly fast, with no off-network worlds at all. A highway strides
/// past places.
const STEP_MIN_FRAC: f64 = 1.1;
const STEP_MAX_FRAC: f64 = 3.5;

/// How much further a starved radial may reach before it is abandoned. Each rung
/// only raises the MAXIMUM hop, never the heading cone, so no rung can produce a
/// corner the fastest hull cannot fly.
const TRUNK_REACH_LADDER: [f64; 4] = [1.0, 1.7, 2.6, 4.0];

/// Centerline samples per spline segment. The route is continuous geometry; this
/// is only how finely the sim and the renderer agree to walk it.
const SAMPLES_PER_SEGMENT: usize = 12;

/// Target share of systems that should end up with NO lane access. Generation is
/// driven by this rather than by a chord count, because a dense galaxy would
/// otherwise quietly become uniformly fast and "off the highway" would stop
/// being a strategic category at all — which would cost the whole information
/// topology its meaning.
const OFF_NETWORK_FLOOR: f64 = 0.30;

/// How far past a ribbon's edge a system still counts as served — the short hop
/// from a world to the highway beside it. Reach is stated against system spacing
/// rather than the well clearance because those are independent design scales.
const SERVED_HOP_FRAC: f64 = 0.30;

// --- GEOMETRY ----------------------------------------------------------------

/// One baked point on a lane's centerline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaneSample {
    pub pos: Vec2,
    /// Unit tangent, pointing along the route's canonical direction.
    pub tangent: Vec2,
    /// Arc length from the route's start.
    pub s: f64,
}

/// How a route came to exist. Routes are not interchangeable: a trunk, an arc
/// and a chord all run past a string of worlds and carry one name the whole way,
/// but a SPUR is a short filament grown from one home to the nearest highway.
/// Judging a spur by "does it serve several systems" asks it to be something it
/// was never built to be — it serves exactly one, and that is its whole job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneKind {
    /// Radial, hub outward.
    Trunk,
    /// A ring crossing the radials, so frontier-to-frontier travel need not go
    /// in to the hub and back out.
    Arc,
    /// Criss-crossing, seeded at a system's own singularity and grown both ways.
    Chord,
    /// A home's own filament to the nearest route. Serves one system by design.
    Spur,
}

/// A named continuous route. Not an `A ↔ B` edge: it runs past several systems,
/// may branch, and fades where the frontier runs out of anchors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lane {
    pub id: u32,
    pub name: String,
    /// What grew it. See `LaneKind` — a spur's contract differs from a trunk's.
    #[serde(default = "LaneKind::default_trunk")]
    pub kind: LaneKind,
    /// The route-shaping points, after the baked centerline's well-clearance
    /// relaxation. The authoritative centerline passes through each one.
    pub control: Vec<Vec2>,
    /// The baked centerline.
    pub samples: Vec<LaneSample>,
    pub half_width: f64,
    /// Routes that faded rather than terminating at an anchor taper their last
    /// stretch, so the network's edge is a gradient rather than a cliff.
    pub tapers: bool,
}

impl LaneKind {
    fn default_trunk() -> Self {
        Self::Trunk
    }
}

impl Lane {
    /// Total arc length.
    pub fn length(&self) -> f64 {
        self.samples.last().map(|s| s.s).unwrap_or(0.0)
    }

    /// The centerline point nearest `p` — as an INTERPOLATED sample on the
    /// polyline through the recorded ones — and the distance to it.
    ///
    /// Measuring to discrete samples is granular by half their spacing, and that
    /// is not a rounding error here: samples sit ~13,600 su apart on a long route
    /// while a ribbon is ~5,900 su half-wide, so a point lying EXACTLY on the
    /// centerline midway between two of them measured ~6,800 away and was ruled
    /// outside the lane it was sitting on. A fleet riding a lane therefore fell
    /// out of it and back in once per segment, at a factor of ten in speed each
    /// time. That is the stutter seen in playtest — it was never lateral drift,
    /// it was the yardstick.
    ///
    /// The ARC POSITION had the same disease one layer down: distance went
    /// continuous but `s` still snapped to the nearer recorded sample, so every
    /// `s`-consumer — the coupled signal's ride length, tripwire earshot, taper
    /// width — sawtoothed once per segment. For a rider that made the coupled
    /// delay step BACKWARD at each boundary (arrival reversals, measured -0.09s),
    /// and the served ghost lurched ~2× its uniform step at every crossing. The
    /// whole sample interpolates now: pos on the segment, `s` and tangent lerped.
    pub fn nearest(&self, p: Vec2) -> Option<(LaneSample, f64)> {
        if self.samples.len() < 2 {
            return self.samples.first().map(|s| (*s, s.pos.distance(p)));
        }
        let mut best: Option<(LaneSample, f64)> = None;
        for w in self.samples.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ab = b.pos - a.pos;
            let len2 = ab.length_sq();
            // Project p onto the segment, clamped to its ends.
            let t = if len2 > 1e-12 { ((p - a.pos).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let pos = a.pos + ab * t;
            let d = p.distance(pos);
            if best.is_none_or(|(_, bd)| d < bd) {
                let lerped = a.tangent + (b.tangent - a.tangent) * t;
                let tangent =
                    if lerped.length_sq() > 1e-12 { lerped.normalized() } else { a.tangent };
                best = Some((LaneSample { pos, tangent, s: a.s + (b.s - a.s) * t }, d));
            }
        }
        best
    }

    /// The ribbon's half-width at arc position `s` — constant, except across a
    /// tapering tail.
    fn half_width_at(&self, s: f64) -> f64 {
        if !self.tapers {
            return self.half_width;
        }
        let len = self.length();
        let taper = len * 0.15;
        if len - s >= taper || taper <= 0.0 {
            self.half_width
        } else {
            // FADES TO A FLOOR, NOT TO NOTHING. Tapering to zero makes the last
            // sliver of a route a ribbon with no inside: a fleet sitting exactly
            // on the centerline at the terminus measures zero offset against a
            // zero half-width and is ruled OUTSIDE the lane it is standing on.
            // The frontier is supposed to thin out, not to stop existing — a
            // route you cannot be on is not a fading route, it is a gap.
            let frac = ((len - s) / taper).clamp(0.0, 1.0);
            self.half_width * TAPER_FLOOR_FRAC.mul_add(1.0 - frac, frac)
        }
    }

    /// The centerline point at arc position `s`, INTERPOLATED between the two
    /// samples bracketing it.
    ///
    /// Snapping to the nearest sample instead makes this useless for subdividing
    /// a run: every point asked for between two samples comes back AS one of
    /// those samples, so a caller trying to lay down closer waypoints silently
    /// gets the original spacing back.
    pub fn at(&self, s: f64) -> Vec2 {
        if self.samples.is_empty() {
            return Vec2::ZERO;
        }
        let i = self.samples.partition_point(|sm| sm.s < s);
        if i == 0 {
            return self.samples[0].pos;
        }
        if i >= self.samples.len() {
            return self.samples[self.samples.len() - 1].pos;
        }
        let (a, b) = (&self.samples[i - 1], &self.samples[i]);
        let span = b.s - a.s;
        if span <= 1e-9 {
            return a.pos;
        }
        a.pos + (b.pos - a.pos) * ((s - a.s) / span)
    }

    /// The centerline points between two arc positions, in travel order — what
    /// makes a fleet follow the curve rather than cut its chord.
    ///
    /// Raw centerline samples, at whatever spacing the bake produced.
    ///
    /// These were briefly SUBDIVIDED, to stop the straight chord between two of
    /// them straying outside the ribbon. Under the old model that mattered — a
    /// fleet outside the ribbon lost its speed — but a fleet now rides a lane
    /// because its route says so, so drifting a few hundred su off the
    /// centerline costs nothing. Subdividing became actively harmful: it put
    /// waypoints ~3.5k apart while a hull at lane speed turns in ~11k, so the
    /// fleet could not physically track them and circled instead of arriving.
    /// Waypoints have to be spaced WIDER than the turning circle, not tighter.
    pub fn span(&self, s0: f64, s1: f64) -> Vec<Vec2> {
        let (lo, hi) = if s0 <= s1 { (s0, s1) } else { (s1, s0) };
        let mut pts: Vec<Vec2> =
            self.samples.iter().filter(|sm| sm.s >= lo && sm.s <= hi).map(|sm| sm.pos).collect();
        if s0 > s1 {
            pts.reverse();
        }
        pts
    }

    /// Is `p` inside the ribbon, and if so on what tangent?
    pub fn contains(&self, p: Vec2) -> Option<LaneHit> {
        let (sample, d) = self.nearest(p)?;
        (d <= self.half_width_at(sample.s)).then_some(LaneHit {
            lane: self.id,
            tangent: sample.tangent,
            s: sample.s,
            offset: d,
        })
    }
}

/// A point's relationship to one lane it lies inside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneHit {
    pub lane: u32,
    pub tangent: Vec2,
    pub s: f64,
    pub offset: f64,
}

/// The whole network.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LaneNetwork {
    pub lanes: Vec<Lane>,
    /// §junction: where two routes run close enough to change from one to the
    /// other, and the stop lists that turn the network into a graph.
    ///
    /// DERIVED from `lanes` — see [`LaneNetwork::rebuild_junctions`]. Carried in
    /// snapshots for completeness and rebuilt by the world's load fixup so
    /// crossing-algorithm improvements also repair existing galaxies.
    #[serde(default)]
    pub junctions: Vec<Junction>,
    /// Per lane, the arc positions that bound its graph edges: every junction on
    /// it plus both ends, sorted. Indices here are the node ids.
    #[serde(default)]
    pub stops: Vec<Vec<f64>>,
}

/// A relay point resolved against the network: where it is, and which routes it
/// is near enough to transmit along.
#[derive(Debug, Clone, PartialEq)]
pub struct Relay {
    pub pos: Vec2,
    /// Lane ids this relay can send along, with its arc position on each.
    pub on: Vec<(u32, f64)>,
}

/// One owned communications structure supplied to the delay field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommSite {
    pub pos: Vec2,
    /// Coverage radius measured in lane-network arc distance.
    pub throw: f64,
    /// Whether warp traffic may board or leave a covered corridor here.
    pub gateway: bool,
}

/// A coupled hull is an endpoint gateway for itself, but contributes no relay
/// coverage. Built infrastructure must already reach the hull's position; the
/// receiver cannot silently double a buoy's displayed throw.
pub const HULL_THROW: f64 = 0.0;

#[derive(Debug, Clone)]
struct LaneRide {
    distance: f64,
    hops: Vec<RideHop>,
}

#[derive(Debug, Clone, Copy)]
struct RideHop {
    to: Vec2,
    lane: u32,
    distance: f64,
}

/// One hop of a signal's journey — what the order graphic traces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hop {
    pub to: Vec2,
    /// The lane ridden, or `None` for a warp hop across open space.
    pub lane: Option<u32>,
    /// CUMULATIVE seconds from send until the signal reaches `to` — straight
    /// from the relay search's arrival times, so an animation that walks the
    /// hops at these timestamps replays the journey the solver actually found
    /// (fast along lanes, slow across the gaps) with nothing re-derived.
    pub t: f64,
}

/// One step of a planned journey: fly to `to`, and if `lane` is set, do it by
/// RIDING that route rather than crossing open space.
///
/// The lane is recorded rather than re-derived, and that is the whole point. It
/// used to be inferred every tick from "am I inside a ribbon and pointing along
/// it?", which answers YES for a fleet merely cutting across one — so a ship
/// crossing a lane diagonally collected the full hyperspace-drive speed for a
/// second and lost it again on the way out, ten times a second. A fleet is on a
/// road because it got on it, not because it happens to be standing on one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    pub to: Vec2,
    /// The route being ridden, or `None` for a warp hop across open space.
    #[serde(default)]
    pub lane: Option<u32>,
}

impl Leg {
    pub fn warp(to: Vec2) -> Self {
        Leg { to, lane: None }
    }
    pub fn riding(to: Vec2, lane: u32) -> Self {
        Leg { to, lane: Some(lane) }
    }
}

/// A place two routes cross closely enough that a fleet inside both ribbons can
/// change from one to the other without leaving the network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Junction {
    /// Indices into `LaneNetwork::lanes`.
    pub a: usize,
    pub b: usize,
    /// Arc position of the crossing along each route.
    pub sa: f64,
    pub sb: f64,
    pub pos: Vec2,
    /// The heading change the crossing demands, folded into `0..=PI/2` because
    /// either direction along a route is equally fast — a fleet joining a lane
    /// head-on and one joining it tail-on face the same turn.
    pub turn: f64,
}

/// Closest points on two finite line segments, returned as
/// `(fraction_on_a, fraction_on_b, point_on_a, point_on_b, distance)`.
///
/// Lane centerlines are baked polylines. Junction discovery must compare their
/// CONTINUOUS segments, not just the baked vertices: a perfectly visible
/// crossing can lie halfway between every sample on both lanes.
fn segment_closest(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (f64, f64, Vec2, Vec2, f64) {
    let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
    let (ad, bd) = (a1 - a0, b1 - b0);
    let denom = cross(ad, bd);
    if denom.abs() > 1e-12 {
        let delta = b0 - a0;
        let (ta, tb) = (cross(delta, bd) / denom, cross(delta, ad) / denom);
        if (0.0..=1.0).contains(&ta) && (0.0..=1.0).contains(&tb) {
            let p = a0 + ad * ta;
            return (ta, tb, p, p, 0.0);
        }
    }

    let project = |p: Vec2, s0: Vec2, s1: Vec2| {
        let d = s1 - s0;
        let t = if d.length_sq() > 1e-12 {
            ((p - s0).dot(d) / d.length_sq()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (t, s0 + d * t)
    };
    let (tb0, qb0) = project(a0, b0, b1);
    let (tb1, qb1) = project(a1, b0, b1);
    let (ta0, qa0) = project(b0, a0, a1);
    let (ta1, qa1) = project(b1, a0, a1);
    [
        (0.0, tb0, a0, qb0, a0.distance(qb0)),
        (1.0, tb1, a1, qb1, a1.distance(qb1)),
        (ta0, 0.0, qa0, b0, qa0.distance(b0)),
        (ta1, 1.0, qa1, b1, qa1.distance(b1)),
    ]
    .into_iter()
    .min_by(|x, y| x.4.total_cmp(&y.4))
    .unwrap()
}

impl LaneNetwork {
    /// Build from lanes, deriving the junction graph.
    pub fn of(lanes: Vec<Lane>) -> Self {
        let mut n = LaneNetwork { lanes, ..Default::default() };
        n.rebuild_junctions();
        n
    }

    /// §comms-infra: resolve a communication site against the network — which routes is it
    /// near enough to transmit along, and where on each?
    ///
    /// Reuses the ribbon's own half-width, the same tolerance a hull needs to
    /// board. "Near enough to count as in the lane" then means one thing across
    /// the whole game rather than two thresholds that quietly drift apart.
    pub fn relay_at(&self, pos: Vec2) -> Relay {
        let on = self
            .lanes
            .iter()
            .filter_map(|l| {
                l.nearest(pos)
                    .filter(|(sm, d)| *d <= l.half_width_at(sm.s))
                    .map(|(sm, _)| (l.id, sm.s))
            })
            .collect();
        Relay { pos, on }
    }

    /// §comms-infra: the fastest way for a SIGNAL to get from `a` to `b`, and the path
    /// it takes.
    ///
    /// Warp everywhere by default. Owned gateways may inject onto or leave a
    /// fully covered lane corridor; repeaters extend that corridor through
    /// junctions but never board traffic. The command center is only the journey
    /// endpoint: home never enters the relay carrier.
    ///
    /// Returns the hops as well as the time, because the order graphic has to
    /// trace the path the signal actually took rather than a straight line to
    /// the destination.
    ///
    /// Cheap by construction, deliberately: this replaces a continuous geometric
    /// search that scanned every sample of every lane twice per query, which the
    /// view filter then ran per history sample per ghost. A handful of relays is
    /// a handful of distance computations.
    pub fn signal(&self, a: Vec2, b: Vec2, c: f64, sites: &[CommSite]) -> (f64, Vec<Hop>) {
        self.signal_routed(a, b, c, sites)
    }

    /// Shortest lane-only path between two resolved relay sites. Junctions cost
    /// their real incoming + outgoing arc distance and no artificial transfer;
    /// this is the metric used by coverage bubbles as well as signal clocks.
    fn lane_ride(&self, from: &Relay, to: &Relay) -> Option<LaneRide> {
        if from.on.is_empty() || to.on.is_empty() {
            return None;
        }
        let mut nodes = vec![from.clone(), to.clone()];
        for j in &self.junctions {
            nodes.push(Relay {
                pos: j.pos,
                on: vec![(self.lanes[j.a].id, j.sa), (self.lanes[j.b].id, j.sb)],
            });
        }
        let edge = |u: usize, v: usize| -> Option<(u32, f64)> {
            let mut best: Option<(u32, f64)> = None;
            for (la, sa) in &nodes[u].on {
                for (lb, sb) in &nodes[v].on {
                    if la != lb {
                        continue;
                    }
                    let distance = (sa - sb).abs();
                    if best.is_none_or(|(_, d)| distance < d) {
                        best = Some((*la, distance));
                    }
                }
            }
            best
        };

        let mut distance = vec![f64::INFINITY; nodes.len()];
        let mut previous: Vec<Option<(usize, u32)>> = vec![None; nodes.len()];
        let mut done = vec![false; nodes.len()];
        distance[0] = 0.0;
        loop {
            let Some((u, du)) = distance
                .iter()
                .enumerate()
                .filter(|(i, d)| !done[*i] && d.is_finite())
                .map(|(i, d)| (i, *d))
                .min_by(|a, b| a.1.total_cmp(&b.1))
            else { break };
            if u == 1 {
                break;
            }
            done[u] = true;
            for v in 0..nodes.len() {
                if done[v] {
                    continue;
                }
                let Some((lane, leg)) = edge(u, v) else { continue };
                if du + leg < distance[v] {
                    distance[v] = du + leg;
                    previous[v] = Some((u, lane));
                }
            }
        }
        if !distance[1].is_finite() {
            return None;
        }
        let mut chain = Vec::new();
        let mut cursor = 1;
        while let Some((prev, lane)) = previous[cursor] {
            chain.push((cursor, lane));
            cursor = prev;
        }
        if cursor != 0 {
            return None;
        }
        chain.reverse();
        let hops = chain
            .into_iter()
            .map(|(node, lane)| RideHop {
                to: nodes[node].pos,
                lane,
                distance: distance[node],
            })
            .collect();
        Some(LaneRide { distance: distance[1], hops })
    }

    /// Find a completely covered corridor between two gateway sites. Repeaters
    /// participate in the coverage search, then disappear from the drawable hop
    /// chain wherever the ride stays on the same lane.
    fn covered_corridor(
        &self,
        start: usize,
        end: usize,
        sites: &[CommSite],
        relays: &[Relay],
    ) -> Option<LaneRide> {
        let n = sites.len();
        let mut edges: Vec<Vec<Option<LaneRide>>> = vec![vec![None; n]; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let Some(ride) = self.lane_ride(&relays[i], &relays[j]) else { continue };
                if ride.distance <= sites[i].throw + sites[j].throw + 1e-6 {
                    edges[i][j] = Some(ride);
                    edges[j][i] = self.lane_ride(&relays[j], &relays[i]);
                }
            }
        }

        let mut distance = vec![f64::INFINITY; n];
        let mut previous = vec![None; n];
        let mut done = vec![false; n];
        distance[start] = 0.0;
        loop {
            let Some((u, du)) = distance
                .iter()
                .enumerate()
                .filter(|(i, d)| !done[*i] && d.is_finite())
                .map(|(i, d)| (i, *d))
                .min_by(|a, b| a.1.total_cmp(&b.1))
            else { break };
            if u == end {
                break;
            }
            done[u] = true;
            for v in 0..n {
                let Some(ride) = &edges[u][v] else { continue };
                if du + ride.distance < distance[v] {
                    distance[v] = du + ride.distance;
                    previous[v] = Some(u);
                }
            }
        }
        if !distance[end].is_finite() {
            return None;
        }
        let mut site_chain = vec![end];
        while let Some(prev) = previous[*site_chain.last().unwrap()] {
            site_chain.push(prev);
        }
        site_chain.reverse();

        let mut hops: Vec<RideHop> = Vec::new();
        let mut offset = 0.0;
        for pair in site_chain.windows(2) {
            let ride = edges[pair[0]][pair[1]].as_ref().unwrap();
            for hop in &ride.hops {
                let distance = offset + hop.distance;
                if let Some(last) = hops.last_mut()
                    && last.lane == hop.lane
                {
                    last.to = hop.to;
                    last.distance = distance;
                } else {
                    hops.push(RideHop { distance, ..*hop });
                }
            }
            offset += ride.distance;
        }
        Some(LaneRide { distance: offset, hops })
    }

    /// Shared gateway-aware signal search. Warp may reach or leave only a
    /// gateway. Lane edges exist only for fully covered corridors; a gap removes
    /// the whole ride edge, so a signal never falls out midway along a lane.
    fn signal_routed(&self, a: Vec2, b: Vec2, c: f64, sites: &[CommSite]) -> (f64, Vec<Hop>) {
        let warp = c * WARP_FACTOR;
        let lane_speed = c * self.signal_factor_on_lane();
        let direct =
            (a.distance(b) / warp, vec![Hop { to: b, lane: None, t: a.distance(b) / warp }]);
        if sites.is_empty() {
            return direct;
        }
        let relays: Vec<Relay> = sites.iter().map(|site| self.relay_at(site.pos)).collect();
        let gateways: Vec<usize> = sites
            .iter()
            .enumerate()
            .filter(|(_, site)| site.gateway)
            .map(|(i, _)| i)
            .collect();
        if gateways.is_empty() {
            return direct;
        }
        let n = gateways.len();
        let mut rides: Vec<Vec<Option<LaneRide>>> = vec![vec![None; n]; n];
        for u in 0..n {
            for v in (u + 1)..n {
                rides[u][v] = self.covered_corridor(gateways[u], gateways[v], sites, &relays);
                rides[v][u] = self.covered_corridor(gateways[v], gateways[u], sites, &relays);
            }
        }

        #[derive(Clone)]
        enum Edge {
            Warp,
            Ride(LaneRide),
        }
        let mut best: Vec<f64> = gateways
            .iter()
            .map(|site| a.distance(sites[*site].pos) / warp)
            .collect();
        let mut prev: Vec<Option<(usize, Edge)>> = vec![None; n];
        let mut done = vec![false; n];
        loop {
            let Some((u, du)) = best
                .iter()
                .enumerate()
                .filter(|(i, _)| !done[*i])
                .map(|(i, d)| (i, *d))
                .filter(|(_, d)| d.is_finite())
                .min_by(|x, y| x.1.total_cmp(&y.1))
            else {
                break;
            };
            done[u] = true;
            for v in 0..n {
                if done[v] {
                    continue;
                }
                let warp_t = sites[gateways[u]].pos.distance(sites[gateways[v]].pos) / warp;
                let (w, edge) = rides[u][v]
                    .as_ref()
                    .map(|ride| (ride.distance / lane_speed, Edge::Ride(ride.clone())))
                    .filter(|(ride_t, _)| *ride_t < warp_t)
                    .unwrap_or((warp_t, Edge::Warp));
                if du + w < best[v] {
                    best[v] = du + w;
                    prev[v] = Some((u, edge));
                }
            }
        }
        let Some((end, t)) = best
            .iter()
            .enumerate()
            .map(|(i, d)| (i, d + sites[gateways[i]].pos.distance(b) / warp))
            .filter(|(_, t)| t.is_finite())
            .min_by(|x, y| x.1.total_cmp(&y.1))
        else {
            return direct;
        };
        if t >= direct.0 {
            return direct;
        }
        let mut steps = Vec::new();
        let mut first = end;
        while let Some((p, edge)) = prev[first].clone() {
            steps.push((p, first, edge));
            first = p;
        }
        steps.reverse();
        let mut hops = vec![Hop {
            to: sites[gateways[first]].pos,
            lane: None,
            t: best[first],
        }];
        for (u, v, edge) in steps {
            match edge {
                Edge::Warp => hops.push(Hop {
                    to: sites[gateways[v]].pos,
                    lane: None,
                    t: best[v],
                }),
                Edge::Ride(ride) => {
                    let start_t = best[u];
                    hops.extend(ride.hops.into_iter().map(|hop| Hop {
                        to: hop.to,
                        lane: Some(hop.lane),
                        t: start_t + hop.distance / lane_speed,
                    }));
                }
            }
        }
        hops.push(Hop { to: b, lane: None, t });
        (t, hops)
    }

    /// Expand the graph-level hops of a signal into the baked geometry of every
    /// lane it rode.
    ///
    /// Delay queries deliberately keep their compact relay/junction path: they
    /// run for every historical sighting and need only the total. The outbound
    /// order animation is low-volume and needs the actual curve, so its path
    /// alone is expanded here. Intermediate samples inherit cumulative times
    /// from the exact graph edge the shortest-path search already priced.
    fn trace_signal_hops(&self, start: Vec2, hops: &[Hop]) -> Vec<Hop> {
        let mut traced = Vec::new();
        let mut edge_start = start;
        let mut edge_start_t = 0.0;
        for hop in hops {
            if let Some(lane_id) = hop.lane
                && let Some(lane) = self.lanes.iter().find(|lane| lane.id == lane_id)
                && let (Some((from, _)), Some((to, _))) =
                    (lane.nearest(edge_start), lane.nearest(hop.to))
            {
                let arc = (to.s - from.s).abs();
                if arc > 1e-9 {
                    let mut samples: Vec<&LaneSample> = lane
                        .samples
                        .iter()
                        .filter(|sample| {
                            let progress = if to.s >= from.s {
                                (sample.s - from.s) / arc
                            } else {
                                (from.s - sample.s) / arc
                            };
                            progress > 1e-9 && progress < 1.0 - 1e-9
                        })
                        .collect();
                    if to.s < from.s {
                        samples.reverse();
                    }
                    for sample in samples {
                        let progress = if to.s >= from.s {
                            (sample.s - from.s) / arc
                        } else {
                            (from.s - sample.s) / arc
                        };
                        traced.push(Hop {
                            to: sample.pos,
                            lane: Some(lane_id),
                            t: edge_start_t + (hop.t - edge_start_t) * progress,
                        });
                    }
                }
            }
            traced.push(*hop);
            edge_start = hop.to;
            edge_start_t = hop.t;
        }
        traced
    }

    /// An outbound order whose RECEIVER is actively riding a lane. Its engaged
    /// drive makes the hull an endpoint gateway for itself, with zero relay
    /// throw; it receives a lane-fast order only when built coverage reaches
    /// that moving endpoint.
    pub fn signal_to_coupled(
        &self,
        a: Vec2,
        p: Vec2,
        c: f64,
        sites: &[CommSite],
    ) -> (f64, Vec<Hop>) {
        let inside = self
            .lanes
            .iter()
            .any(|lane| lane.nearest(p).is_some_and(|(on, d)| d <= lane.half_width_at(on.s)));
        if !inside {
            return self.signal(a, p, c, sites);
        }
        // §coupled: an engaged hyperdrive is a terminal wound into the medium —
        // the same coupling that carries its reports home receives an order at
        // the hull. It joins the graph as a zero-throw gateway, so built wire
        // may terminate at the ship but the ship cannot extend that wire beyond
        // the selected relay's displayed coverage boundary.
        let mut with_hull = sites.to_vec();
        with_hull.push(CommSite { pos: p, throw: HULL_THROW, gateway: true });
        self.signal_routed(a, p, c, &with_hull)
    }

    /// §coupled: the signal delay from a fleet that is RIDING A LANE to `b`.
    ///
    /// A hull with its hyperspace drive engaged is a terminal inside the
    /// medium, so its own report may ride covered lane wire before exiting
    /// toward the receiver. The outbound direction mirrors this
    /// (`signal_to_coupled`): the same coupling makes the hull its own zero-throw
    /// gateway, so an order rides only when the owner's built coverage reaches
    /// the hull itself.
    ///
    /// The hull joins the lane graph as its origin gateway for this query. Its
    /// report can follow covered wire through junctions, then leave hyperspace
    /// only at a physical gateway.
    pub fn signal_coupled(&self, p: Vec2, b: Vec2, c: f64, sites: &[CommSite]) -> f64 {
        let inside = self
            .lanes
            .iter()
            .any(|lane| lane.nearest(p).is_some_and(|(on, d)| d <= lane.half_width_at(on.s)));
        if !inside {
            return self.signal(p, b, c, sites).0;
        }
        let mut with_hull = sites.to_vec();
        with_hull.push(CommSite { pos: p, throw: HULL_THROW, gateway: true });
        self.signal_routed(p, b, c, &with_hull).0
    }

    /// §coupled: the delay for a viewer to HEAR a coupled hull at `p` through
    /// their lane listening posts (`ears`), then get the report home to `b`.
    ///
    /// The wake rides the lane to the nearest post within listening range, and
    /// the post relays home through the ordinary signal path (gateways, wire,
    /// then warp).
    /// `INFINITY` when no post hears it — the caller minimises this against
    /// passive light, so no ears simply means no advantage.
    pub fn signal_heard(
        &self,
        p: Vec2,
        ears: &[Relay],
        b: Vec2,
        c: f64,
        sites: &[CommSite],
    ) -> f64 {
        let lane_speed = c * self.signal_factor_on_lane();
        let mut best = f64::INFINITY;
        for l in &self.lanes {
            let Some((on, d)) = l.nearest(p) else { continue };
            if d > l.half_width_at(on.s) {
                continue; // the hull is not coupled to THIS lane
            }
            for ear in ears {
                for (lid, s_ear) in &ear.on {
                    if *lid != l.id {
                        continue;
                    }
                    let along = (on.s - s_ear).abs();
                    if along > crate::emplace::LANE_LISTEN_RANGE {
                        continue; // a wake attenuates — out of earshot
                    }
                    let t =
                        along / lane_speed + self.signal_coupled(ear.pos, b, c, sites);
                    best = best.min(t);
                }
            }
        }
        best
    }

    /// §junction: find every crossing and lay out the graph.
    ///
    /// Two routes cross where their ribbons overlap — a fleet standing there is
    /// inside both, so it can change from one to the other without ever dropping
    /// out of the network.
    ///
    /// Compare continuous baked SEGMENTS, never just their vertices. Vertex-only
    /// discovery missed crossings that fell between samples; the route planner
    /// then stayed on an arc almost all the way around to the next registered
    /// junction instead of making the obvious corner the map showed.
    ///
    /// A long shallow overlap is ONE crossing, not one per segment pair:
    /// adjacent overlapping pairs are collapsed to their closest handoff, or
    /// two routes running alongside each other would contribute hundreds of
    /// identical junctions and swamp the search.
    pub fn rebuild_junctions(&mut self) {
        #[derive(Clone, Copy)]
        struct Candidate {
            ia: usize,
            ib: usize,
            sa: f64,
            sb: f64,
            pa: Vec2,
            pb: Vec2,
            ta: Vec2,
            tb: Vec2,
            distance: f64,
        }

        self.junctions.clear();
        let n = self.lanes.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&self.lanes[i], &self.lanes[j]);
                let mut candidates = Vec::new();
                for (ia, aw) in a.samples.windows(2).enumerate() {
                    for (ib, bw) in b.samples.windows(2).enumerate() {
                        let (fa, fb, pa, pb, distance) =
                            segment_closest(aw[0].pos, aw[1].pos, bw[0].pos, bw[1].pos);
                        let sa = aw[0].s + (aw[1].s - aw[0].s) * fa;
                        let sb = bw[0].s + (bw[1].s - bw[0].s) * fb;
                        if distance > a.half_width_at(sa) + b.half_width_at(sb) {
                            continue;
                        }
                        let ta = (aw[0].tangent + (aw[1].tangent - aw[0].tangent) * fa).normalized();
                        let tb = (bw[0].tangent + (bw[1].tangent - bw[0].tangent) * fb).normalized();
                        candidates.push(Candidate {
                            ia,
                            ib,
                            sa,
                            sb,
                            pa,
                            pb,
                            ta,
                            tb,
                            distance,
                        });
                    }
                }

                // Connected components in segment-pair space collapse the many
                // adjacent candidates around one crossing/overlap while keeping
                // two geometrically separate crossings between the same routes.
                while let Some(seed) = candidates.pop() {
                    let mut cluster = vec![seed];
                    loop {
                        let before = candidates.len();
                        let mut k = 0;
                        while k < candidates.len() {
                            let c = candidates[k];
                            let touches = cluster.iter().any(|x| {
                                x.ia.abs_diff(c.ia) <= 1 && x.ib.abs_diff(c.ib) <= 1
                            });
                            if touches {
                                cluster.push(candidates.swap_remove(k));
                            } else {
                                k += 1;
                            }
                        }
                        if candidates.len() == before {
                            break;
                        }
                    }
                    let best = cluster
                        .into_iter()
                        .min_by(|x, y| x.distance.total_cmp(&y.distance))
                        .unwrap();
                    let dot = best.ta.dot(best.tb).abs().clamp(0.0, 1.0);
                    self.junctions.push(Junction {
                        a: i,
                        b: j,
                        sa: best.sa,
                        sb: best.sb,
                        pos: (best.pa + best.pb) * 0.5,
                        turn: dot.acos(),
                    });
                }
            }
        }
        self.junctions.sort_by(|x, y| {
            x.a.cmp(&y.a)
                .then(x.b.cmp(&y.b))
                .then(x.sa.total_cmp(&y.sa))
                .then(x.sb.total_cmp(&y.sb))
        });
        // Stops per lane: every junction on it, plus both ends.
        self.stops = self
            .lanes
            .iter()
            .enumerate()
            .map(|(li, l)| {
                let mut v: Vec<f64> = vec![0.0, l.length()];
                for j in &self.junctions {
                    if j.a == li {
                        v.push(j.sa);
                    }
                    if j.b == li {
                        v.push(j.sb);
                    }
                }
                v.sort_by(f64::total_cmp);
                v.dedup_by(|x, y| (*x - *y).abs() < 1.0);
                v
            })
            .collect();
    }


    /// §junction: the fastest way across the NETWORK, not along one route.
    ///
    /// Costs are in DEEP-EQUIVALENT DISTANCE: warp travel counts its
    /// own length, riding a lane counts `length / lane_ratio`. That works because
    /// total time is `(d_deep + d_lane / LANE_MULT) / (base × H)` — the hull's
    /// own speed factors straight out, so a Scout and a Titan want the same path
    /// and the graph never has to be re-solved per hull.
    ///
    /// `transfer` is what a crossing costs in the same units. A hull pays: it has
    /// to swing onto the new heading, and past the alignment gate it does that at
    /// a tenth its lane speed and a tenth its turning radius. A SIGNAL pays
    /// nothing — it has no turning circle to arc through — which is why
    /// information is better connected across this network than freight is.
    ///
    /// Returns the node path, or `None` when flying straight beats the network.
    /// §junction: the fastest way across the NETWORK, as (lane index, arc
    /// position) pairs — resolved places, not graph node ids.
    ///
    /// The graph's permanent stops are junctions and endpoints, and for a while
    /// those were the ONLY places a ride could begin or end. That is wrong for
    /// any destination beside the middle of a lane: with no stop near it, the
    /// path exits at the far junction instead, and a fleet visibly sails PAST
    /// where it was sent, still on the lane. So each query adds two stops of its
    /// own — the origin's and destination's projections onto every lane — and a
    /// ride can now start and end at the nearest point worth using.
    fn best_path(&self, from: Vec2, to: Vec2, lane_ratio: f64, transfer: f64) -> Option<Vec<(usize, f64)>> {
        if self.lanes.is_empty() {
            return None;
        }
        // Augment the standing stops with this query's own on/off points.
        let mut stops: Vec<Vec<f64>> = self.stops.clone();
        for (li, l) in self.lanes.iter().enumerate() {
            for p in [from, to] {
                if let Some((sm, _)) = l.nearest(p) {
                    stops[li].push(sm.s);
                }
            }
            stops[li].sort_by(f64::total_cmp);
            stops[li].dedup_by(|x, y| (*x - *y).abs() < 1.0);
        }
        let offset: Vec<usize> = stops
            .iter()
            .scan(0usize, |acc, v| {
                let o = *acc;
                *acc += v.len();
                Some(o)
            })
            .collect();
        let total: usize = stops.iter().map(Vec::len).sum();
        let locate = |node: usize| -> (usize, usize) {
            let li = offset.iter().rposition(|o| *o <= node).unwrap_or(0);
            (li, node - offset[li])
        };
        let stop_at = |li: usize, s: f64| -> usize {
            stops[li]
                .iter()
                .enumerate()
                .min_by(|x, y| (x.1 - s).abs().total_cmp(&(y.1 - s).abs()))
                .map_or(0, |(k, _)| k)
        };
        let mut pos: Vec<Vec2> = Vec::with_capacity(total);
        for (li, l) in self.lanes.iter().enumerate() {
            for st in &stops[li] {
                pos.push(l.at(*st));
            }
        }
        let direct = from.distance(to);
        let mut dist: Vec<f64> = pos.iter().map(|p| from.distance(*p)).collect();
        let mut prev: Vec<Option<usize>> = vec![None; total];
        let mut done = vec![false; total];
        loop {
            let Some((u, du)) = dist
                .iter()
                .enumerate()
                .filter(|(i, _)| !done[*i])
                .map(|(i, d)| (i, *d))
                .filter(|(_, d)| d.is_finite() && *d < direct)
                .min_by(|x, y| x.1.total_cmp(&y.1))
            else {
                break;
            };
            done[u] = true;
            let (lane, k) = locate(u);
            let relax = |v: usize, w: f64, dist: &mut Vec<f64>, prev: &mut Vec<Option<usize>>| {
                if du + w < dist[v] {
                    dist[v] = du + w;
                    prev[v] = Some(u);
                }
            };
            for kk in [k + 1, k.wrapping_sub(1)] {
                if kk < stops[lane].len() {
                    let w = (stops[lane][kk] - stops[lane][k]).abs() / lane_ratio.max(1e-9);
                    relax(offset[lane] + kk, w, &mut dist, &mut prev);
                }
            }
            for j in &self.junctions {
                let (here, there) = if j.a == lane && stop_at(j.a, j.sa) == k {
                    (true, offset[j.b] + stop_at(j.b, j.sb))
                } else if j.b == lane && stop_at(j.b, j.sb) == k {
                    (true, offset[j.a] + stop_at(j.a, j.sa))
                } else {
                    (false, 0)
                };
                if here {
                    relax(there, transfer * j.turn, &mut dist, &mut prev);
                }
            }
        }
        let (end, t) = dist
            .iter()
            .enumerate()
            .map(|(i, d)| (i, d + pos[i].distance(to)))
            .filter(|(_, c)| c.is_finite())
            .min_by(|x, y| x.1.total_cmp(&y.1))?;
        if t >= direct {
            return None; // the straight run wins
        }
        let mut chain = vec![end];
        while let Some(p) = prev[*chain.last().unwrap()] {
            chain.push(p);
        }
        chain.reverse();
        if chain.len() < 2 {
            return None; // touching one point of the network is not a ride
        }
        Some(
            chain
                .into_iter()
                .map(|n| {
                    let (li, k) = locate(n);
                    (li, stops[li][k])
                })
                .collect(),
        )
    }


    /// Every lane whose ribbon covers `p`. A point may lie in several at a
    /// junction; that is ordinary, and never additive (see `speed_factor`).
    pub fn hits(&self, p: Vec2) -> Vec<LaneHit> {
        self.lanes.iter().filter_map(|l| l.contains(p)).collect()
    }

    /// THE SPEED FACTOR AT A POINT, for a fleet heading in `dir`.
    ///
    /// `max` over covering lanes, **never a sum**. Overlapping ribbons do not
    /// accumulate, because strain is a property of the substrate at a point
    /// rather than a stack of bonuses — two panes of glass do not double the
    /// refractive index. Stating it as a property of the medium makes
    /// non-accumulation definitional rather than a special case to remember.
    ///
    /// Alignment-gated: a fleet cutting ACROSS a lane earns nothing, which is
    /// what makes the bands mean something and what makes a turn cost its arc.
    pub fn speed_factor(&self, p: Vec2, dir: Vec2) -> f64 {
        let d = dir.normalized();
        if d.length_sq() < 0.5 {
            return 1.0; // stationary: no heading to align
        }
        let aligned = self
            .hits(p)
            .into_iter()
            .filter(|h| {
                // Either direction along the route counts: a lane is not a
                // current, so both bands are equally fast.
                h.tangent.dot(d).abs() >= ALIGN_TOLERANCE_RAD.cos()
            })
            .count();
        if aligned > 0 { LANE_MULT } else { 1.0 }
    }

    /// Whether `p` lies inside any ribbon at all, ignoring heading. Used for
    /// presentation and for the delay field, which has no heading.
    pub fn on_lane(&self, p: Vec2) -> bool {
        self.lanes.iter().any(|l| l.contains(p).is_some())
    }

    /// Minimum curvature radius across every route — the navigability check. A
    /// lane whose curve is tighter than the fastest hull's turning circle is a
    /// lane that hull physically cannot follow, which is a generation bug.
    pub fn min_curvature_radius(&self) -> f64 {
        self.lanes
            .iter()
            .flat_map(|l| {
                l.samples.windows(3).map(|w| {
                    let (a, b, c) = (w[0].pos, w[1].pos, w[2].pos);
                    menger_radius(a, b, c)
                })
            })
            .fold(f64::INFINITY, f64::min)
    }
}

/// Radius of the circle through three points (Menger curvature's reciprocal).
/// `INFINITY` for collinear points, which is the correct "no curvature" answer.
fn menger_radius(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    let (ab, bc, ca) = (a.distance(b), b.distance(c), c.distance(a));
    let area2 = ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs();
    if area2 <= f64::EPSILON {
        return f64::INFINITY;
    }
    ab * bc * ca / (2.0 * area2)
}

// --- SPLINE ------------------------------------------------------------------

/// Catmull–Rom through `pts`, sampled into a baked centerline. Chosen over
/// Bézier because it interpolates its control points: the route provably passes
/// through the anchors generation chose, so the drawn geometry and the
/// mechanical geometry are the same object.
/// Test-only re-export of the baker, so server tests can build a hand-made lane
/// without reaching into generation.
pub fn bake_for_tests(pts: &[Vec2]) -> Vec<LaneSample> {
    bake(pts)
}

fn bake(pts: &[Vec2]) -> Vec<LaneSample> {
    if pts.len() < 2 {
        return Vec::new();
    }
    // Duplicate the ends so the first and last segments are defined.
    let mut ext = Vec::with_capacity(pts.len() + 2);
    ext.push(pts[0] + (pts[0] - pts[1]));
    ext.extend_from_slice(pts);
    ext.push(pts[pts.len() - 1] + (pts[pts.len() - 1] - pts[pts.len() - 2]));

    let mut out: Vec<LaneSample> = Vec::new();
    let mut s = 0.0;
    for w in ext.windows(4) {
        let (p0, p1, p2, p3) = (w[0], w[1], w[2], w[3]);
        for i in 0..SAMPLES_PER_SEGMENT {
            let t = i as f64 / SAMPLES_PER_SEGMENT as f64;
            let pos = catmull_rom(p0, p1, p2, p3, t);
            let tangent = catmull_rom_tangent(p0, p1, p2, p3, t).normalized();
            if let Some(prev) = out.last() {
                s += prev.pos.distance(pos);
            }
            out.push(LaneSample { pos, tangent, s });
        }
    }
    // Close the final point.
    let last = *pts.last().unwrap();
    if let Some(prev) = out.last() {
        s += prev.pos.distance(last);
        let tangent = out.last().unwrap().tangent;
        out.push(LaneSample { pos: last, tangent, s });
    }
    out
}

/// CENTRIPETAL knot spacing (α = 0.5), not uniform.
///
/// A growth walk hops anchor to anchor, so consecutive control points sit
/// anywhere from `STEP_MIN_FRAC` to `STEP_MAX_FRAC` apart — better than a 2×
/// spread. Uniform parameterization spends equal spline parameter on a short
/// segment and a long one, which bulges the curve outside its own control
/// polygon wherever that scale changes abruptly. Not cosmetic here: the bulge
/// is what put `The Long Reach` at a 23.7k turning radius when its polygon was
/// a comfortable 83k, and what bowed `Halloran Drift` into a gravity well its
/// control points cleared. Centripetal knots are the standard remedy — provably
/// free of cusps and self-intersection, and they hold the curve near its
/// polygon, which is the property both invariants lean on.
fn knots(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> [f64; 4] {
    // A zero-length segment would collapse a knot interval and divide by zero;
    // the floor keeps a duplicated control point survivable.
    let step = |a: Vec2, b: Vec2| a.distance(b).max(1e-9).sqrt();
    let k1 = step(p0, p1);
    let k2 = k1 + step(p1, p2);
    [0.0, k1, k2, k2 + step(p2, p3)]
}

/// Barry–Goldman pyramidal evaluation. `t` is the caller's 0..1 position along
/// the p1→p2 span, mapped onto the centripetal knot interval [k1, k2].
fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f64) -> Vec2 {
    let [k0, k1, k2, k3] = knots(p0, p1, p2, p3);
    let t = k1 + (k2 - k1) * t;
    let lerp = |a: Vec2, b: Vec2, ta: f64, tb: f64| {
        let d = tb - ta;
        if d.abs() < 1e-12 {
            return a;
        }
        a * ((tb - t) / d) + b * ((t - ta) / d)
    };
    let a1 = lerp(p0, p1, k0, k1);
    let a2 = lerp(p1, p2, k1, k2);
    let a3 = lerp(p2, p3, k2, k3);
    lerp(lerp(a1, a2, k0, k2), lerp(a2, a3, k1, k3), k1, k2)
}

/// Central difference of the curve above. The closed form under Barry–Goldman
/// is a page of algebra for a value only ever consumed normalized, as a heading.
fn catmull_rom_tangent(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f64) -> Vec2 {
    const H: f64 = 1e-4;
    let (lo, hi) = ((t - H).max(0.0), (t + H).min(1.0));
    let d = catmull_rom(p0, p1, p2, p3, hi) - catmull_rom(p0, p1, p2, p3, lo);
    if d.length_sq() < 1e-18 { p2 - p1 } else { d }
}

// --- TRANSIT -----------------------------------------------------------------

/// §hyperspace: HOW FAST A FLEET GOES HERE.
///
/// Hyperspace is an OVERLAY on the same coordinates, not a separate place — a
/// ship with its drive on is in it, anywhere, except inside a gravity well.
/// So the three regimes are a property of where you are and what your drive is
/// doing, resolved per tick rather than being a mode you enter.
pub struct TransitEnv<'a> {
    pub lanes: &'a LaneNetwork,
    /// Star positions and their hyperlimits. Inside one, the drive will not
    /// light — which is what keeps every existing system-scale mechanic
    /// (blockade, siege, platforms, docking, the ground war) untouched, and what
    /// stops a besieged fleet simply hyperspacing out.
    pub wells: &'a [(Vec2, f64)],
}

/// Which DRIVE is carrying a fleet. Three of them, each an order of magnitude
/// apart, and until this existed the only way to tell them apart on the map was
/// to eyeball how fast the sprite was crawling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Regime {
    /// THRUSTERS. Sublight manoeuvring — inside a gravity well, or with nothing
    /// else lit.
    #[default]
    Thrusters,
    /// WARP DRIVE. Open space at `WARP_FACTOR` times thrusters, and it can be
    /// flown anywhere: no lane required, no alignment to hold.
    Warp,
    /// HYPERSPACE DRIVE. Riding a lane, another `LANE_MULT` on top of warp. It
    /// only engages near a lane — the drive is what gets a hull ONTO the road,
    /// so away from one there is nothing for it to bite on.
    Hyperspace,
}

impl Regime {
    /// serde default for `DriveState::Dropping::from` — snapshots that predate
    /// the field load as a warp drop, the harmless guess.
    pub fn warp() -> Self {
        Regime::Warp
    }

    /// The label the map uses. Kept here so the sim owns the vocabulary and the
    /// client cannot drift from it.
    pub fn label(self) -> &'static str {
        match self {
            Regime::Thrusters => "impulse",
            Regime::Warp => "warp",
            Regime::Hyperspace => "hyperspace",
        }
    }
}

impl TransitEnv<'_> {
    /// Which layer a fleet at `p` heading `dir` is actually moving through.
    /// Derived from the same factor that sets its speed, so the badge on the map
    /// and the distance covered can never disagree.
    pub fn regime(&self, p: Vec2, dir: Vec2, drive_on: bool) -> Regime {
        let f = self.factor(p, dir, drive_on);
        if f >= WARP_FACTOR * LANE_MULT - 1e-9 {
            Regime::Hyperspace
        } else if f >= WARP_FACTOR - 1e-9 {
            Regime::Warp
        } else {
            Regime::Thrusters
        }
    }

    /// Is `p` inside a gravity well?
    pub fn in_well(&self, p: Vec2) -> bool {
        self.wells.iter().any(|(c, r)| p.distance(*c) <= *r)
    }

    /// The speed multiplier on a hull's base speed: `1` normal, `H` in open
    /// hyperspace, `H × LANE_MULT` on an aligned lane.
    ///
    /// Never additive across overlapping lanes — see `LaneNetwork::speed_factor`.
    pub fn factor(&self, p: Vec2, dir: Vec2, drive_on: bool) -> f64 {
        if !drive_on || self.in_well(p) {
            return 1.0;
        }
        WARP_FACTOR * self.lanes.speed_factor(p, dir)
    }

    /// The tightest circle a fleet moving at `speed` can turn. Constant speed is
    /// preserved — only heading is rate-limited — so `t = d/v` still holds along
    /// the flown path and `uncertainty = age × speed` stays exact.
    pub fn turn_radius(speed: f64) -> f64 {
        TURN_K * speed
    }
}

// --- THE DELAY FIELD ---------------------------------------------------------
//
// Information delay stops being `distance / c` and becomes a shortest-TIME path
// through a medium whose speed varies — the classic variable-speed routing
// problem (Fermat's principle; a lane boundary refracts the optimal route
// exactly as Snell's law says it should).
//
// THE GUARANTEE THAT MAKES THIS SAFE. Strain scales signals and hulls by the
// same factor, so `c / v` is identical in every regime. A signal can therefore
// always take a ship's OWN route and beat it by that ratio — meaning orders
// outrun fleets on any path a fleet can fly, with no special case and without
// ever rescaling `c`. That is the whole reason the uniform-strain reading was
// worth insisting on.

impl LaneNetwork {
    /// Signal speed at a point, as a multiple of normal-space `c`. Direction-free:
    /// a lane is not a current, so it carries information equally both ways —
    /// which is also why `delay(a→b) == delay(b→a)` and one field serves both
    /// outbound orders and inbound reports.
    pub fn signal_factor(&self, p: Vec2) -> f64 {
        if self.on_lane(p) { self.signal_factor_on_lane() } else { WARP_FACTOR }
    }

    /// Signal speed inside a ribbon, as a multiple of normal-space `c`.
    fn signal_factor_on_lane(&self) -> f64 {
        WARP_FACTOR * LANE_MULT
    }

    /// INFORMATION DELAY between two points, in seconds, at normal-space `c`.
    ///
    /// Two candidate routes, cheapest wins:
    ///   * direct through warp, and
    ///   * hop to a lane, run along it, hop off.
    ///
    /// The lane network never moves, so the middle leg is a static all-pairs
    /// problem that can be precomputed at generation; this walks it directly,
    /// which is honest and fast enough at galaxy scale. `LANE_ENTRY_CANDIDATES`
    /// bounds the search — entering a lane far away costs distance linearly
    /// while the saving is bounded, so the optimal entry is always local.
    pub fn delay(&self, a: Vec2, b: Vec2, c: f64) -> f64 {
        let warp = c * WARP_FACTOR;
        let direct = a.distance(b) / warp;
        let lane_speed = c * self.signal_factor_on_lane();
        let ratio = lane_speed / warp;
        let Some(pairs) = self.best_path(a, b, ratio, 0.0) else {
            return direct;
        };
        let mut t = a.distance(self.lanes[pairs[0].0].at(pairs[0].1)) / warp;
        for w in pairs.windows(2) {
            if w[0].0 == w[1].0 {
                t += (w[1].1 - w[0].1).abs() / lane_speed;
            }
        }
        let last = pairs[pairs.len() - 1];
        t += self.lanes[last.0].at(last.1).distance(b) / warp;
        t.min(direct)
    }
}

/// §hyperspace: the INFORMATION DELAY FIELD — everything the view filter needs to
/// answer "how stale is this?" in one object.
///
/// It replaces the bare `c` that used to be threaded through the filters. Delay
/// is no longer `distance / c` but a shortest-TIME path through a medium whose
/// speed varies, so the network has to travel with the constant.
#[derive(Clone, Copy)]
pub struct DelayField<'a> {
    pub lanes: &'a LaneNetwork,
    /// The viewer's own ranged comm sites. Per-player because a corridor is
    /// infrastructure they built; rival sites carry nothing of theirs. Empty
    /// means every signal runs at warp.
    #[allow(clippy::doc_markdown)]
    pub sites: &'a [CommSite],
    /// Normal-space `c`, still the anchor every other speed is expressed against.
    pub c: f64,
}

impl DelayField<'_> {
    /// Solve when a signal sent from `source` meets a receiver moving at a
    /// constant velocity.
    ///
    /// The first delay reaches the receiver's position at issue time. Each
    /// refinement advances the receiver by that delay and re-routes to the new
    /// point. Four passes are ample because every signal regime outruns every
    /// hull; stop earlier once another pass changes the clock by less than one
    /// simulation tick. `coupled` is deliberately fixed by the receiver's drive
    /// state at issue time rather than guessed again from each extrapolated point.
    /// `delay_factor` carries regional command-tempo effects without changing
    /// the path geometry.
    pub fn meeting_delay(
        &self,
        source: Vec2,
        receiver_pos: Vec2,
        receiver_vel: Vec2,
        coupled: bool,
        delay_factor: f64,
    ) -> (f64, Vec2) {
        let travel_time = |target| {
            let raw = if coupled {
                self.to_coupled(source, target)
            } else {
                self.between(source, target)
            };
            raw * delay_factor
        };
        let mut delay = travel_time(receiver_pos);
        let mut meeting = receiver_pos;
        for _ in 0..4 {
            meeting = receiver_pos + receiver_vel * delay;
            let next = travel_time(meeting);
            let converged = (next - delay).abs() < crate::config::DT;
            delay = next;
            if converged {
                break;
            }
        }
        (delay, meeting)
    }

    /// Seconds for information to get from `a` to `b`.
    ///
    /// Symmetric, because a lane is not a current and carries information equally
    /// both ways — which is what lets one field serve outbound orders and inbound
    /// reports alike.
    pub fn between(&self, a: Vec2, b: Vec2) -> f64 {
        self.lanes.signal(a, b, self.c, self.sites).0
    }

    /// The same journey, with the HOPS it takes — what the order graphic traces
    /// so the line on the map is the path the signal actually flew.
    pub fn path(&self, a: Vec2, b: Vec2) -> Vec<Hop> {
        let hops = self.lanes.signal(a, b, self.c, self.sites).1;
        self.lanes.trace_signal_hops(a, &hops)
    }

    /// Seconds for an outbound order whose receiver is actively coupled to a
    /// lane. Off-lane receivers must use [`Self::between`] and a physical relay
    /// as their hyperspace exit.
    pub fn to_coupled(&self, a: Vec2, p: Vec2) -> f64 {
        self.lanes.signal_to_coupled(a, p, self.c, self.sites).0
    }

    /// The drawable form of [`Self::to_coupled`].
    pub fn path_to_coupled(&self, a: Vec2, p: Vec2) -> Vec<Hop> {
        let hops = self.lanes.signal_to_coupled(a, p, self.c, self.sites).1;
        self.lanes.trace_signal_hops(a, &hops)
    }

    /// §coupled: `between`, for a sender RIDING A LANE — its report goes out
    /// through the medium it is coupled to. See `LaneNetwork::signal_coupled`.
    pub fn from_coupled(&self, p: Vec2, b: Vec2) -> f64 {
        self.lanes.signal_coupled(p, b, self.c, self.sites)
    }

    /// §coupled: the delay to HEAR a coupled hull at `p` through the viewer's
    /// lane listening posts. INFINITY with no post in earshot — minimise against
    /// `between`. See `LaneNetwork::signal_heard`.
    pub fn heard(&self, p: Vec2, b: Vec2, ears: &[Relay]) -> f64 {
        self.lanes.signal_heard(p, ears, b, self.c, self.sites)
    }
}

// --- ROUTING -----------------------------------------------------------------
//
// A lane is a CURVE, so riding one means following its centerline — not flying a
// straight line and hoping to clip it at a useful angle. A move therefore
// resolves to a ROUTE: a deep-space leg to an entry point, the lane's own
// samples through to an exit, and a deep-space leg to the destination.
//
// Same shape as the delay field, because it is the same problem: shortest TIME
// through a medium whose speed varies. The difference is only that a ship cares
// about the geometry it must physically fly, while a signal only needed the
// duration.

impl LaneNetwork {
    /// Plan a route from `from` to `to`.
    ///
    /// Returns the waypoints to fly, EXCLUDING the start and always ending at
    /// `to`. A direct run returns just `[to]` — the common case near home, and
    /// the fallback whenever no lane actually saves time.
    ///
    /// `v_deep` and `v_lane` are the fleet's speeds in warp and on a
    /// lane; the comparison is made at the speeds THIS fleet can actually reach,
    /// so a slow hauler and a fast raider can legitimately choose different
    /// routes over the same pair of points.
    /// §junction: the waypoints of the fastest way from `from` to `to`.
    ///
    /// Searches the whole NETWORK rather than each route in isolation. The old
    /// version tried every lane alone and kept the best, so a path was always
    /// "hop out to one lane, ride it, hop to the destination" — it could not
    /// change lanes where two of them cross. Measured on a long leg, those two
    /// hops were a third of the distance and eighty-three percent of the clock:
    /// riding is nearly free, so the trip is priced almost entirely by whatever
    /// is NOT on a lane, and the one thing the planner could not do was shorten
    /// that part.
    ///
    /// Empty means fly straight — no path through the network beat the direct
    /// run, which is the honest answer for a short hop.
    pub fn route(&self, from: Vec2, to: Vec2, v_deep: f64, v_lane: f64) -> Vec<Leg> {
        let ratio = (v_lane / v_deep.max(1e-9)).max(1.0);
        let transfer = TURN_K * v_deep;
        let Some(pairs) = self.best_path(from, to, ratio, transfer) else {
            return Vec::new();
        };
        let mut legs: Vec<Leg> = Vec::new();
        // GETTING ON: reach the first point of the ride. This is a warp hop only
        // when the hull starts outside the ribbon.
        if let Some(&(l0, s0)) = pairs.first() {
            let entry = self.lanes[l0].at(s0);
            if entry.distance(from) > 1.0 {
                // A parked hull can be off the centerline while still inside
                // the ribbon. It is already coupled there, so converging on the
                // lane's centerline must not masquerade as an open-space hop and
                // force the drive down before its next order.
                legs.push(if self.lanes[l0].contains(from).is_some() {
                    Leg::riding(entry, self.lanes[l0].id)
                } else {
                    Leg::warp(entry)
                });
            }
        }
        for w in pairs.windows(2) {
            let (la, sa) = w[0];
            let (lb, sb) = w[1];
            if la == lb {
                // RIDING: the centerline between the two stops, tagged with the
                // road it belongs to, ending exactly at the off point.
                let id = self.lanes[la].id;
                legs.extend(self.lanes[la].span(sa, sb).into_iter().map(|p| Leg::riding(p, id)));
                legs.push(Leg::riding(self.lanes[la].at(sb), id));
            } else {
                // CHANGING ROADS at a crossing.
                legs.push(Leg::riding(self.lanes[lb].at(sb), self.lanes[lb].id));
            }
        }
        legs.dedup_by(|x, y| x.to.distance(y.to) < 1.0 && x.lane == y.lane);
        while legs.first().is_some_and(|l| l.to.distance(from) < 1.0) {
            legs.remove(0);
        }
        // GETTING OFF — unless the destination itself is still inside the
        // ribbon. The old unconditional warp leg made a fleet drop its
        // hyperspace drive for the last few pixels of an otherwise lane-bound
        // trip, then sit disconnected while the player waited to issue the next
        // order. A destination inside the final ribbon is a lane arrival: fly
        // the short remainder without leaving the medium.
        let arrival_lane = pairs
            .last()
            .and_then(|(li, _)| self.lanes[*li].contains(to).map(|_| self.lanes[*li].id));
        legs.push(match arrival_lane {
            Some(lane) => Leg::riding(to, lane),
            None => Leg::warp(to),
        });
        legs.dedup_by(|x, y| x.to.distance(y.to) < 1.0 && x.lane == y.lane);
        legs
    }
}

// --- GENERATION --------------------------------------------------------------

/// What generation needs to know about a system: where it is, and how big a
/// berth its gravity well demands.
#[derive(Debug, Clone, Copy)]
pub struct LaneAnchor {
    pub pos: Vec2,
}

/// Deterministically generate the network.
///
/// The pipeline, in order: seed trunks toward each home (a FAIRNESS requirement,
/// since lanes are the information network too and an off-network home plays a
/// materially different game), grow each as a heading-constrained outward walk,
/// branch, add arc routes for lateral movement, add criss-crossing chords until
/// the off-network floor is reached, then offset from wells, repair curvature,
/// and name.
pub fn generate(
    seed: u64,
    hub: Vec2,
    anchors: &[LaneAnchor],
    homes: &[Vec2],
    radius: f64,
) -> LaneNetwork {
    let mut rng = Rng::new(seed ^ 0x4C41_4E45_5F47_454E); // "LANE_GEN"
    let spacing = median_spacing(anchors);
    let half_width = ribbon_half_width(spacing);
    let mut lanes: Vec<Lane> = Vec::new();
    let mut next_id = 0u32;

    // 1 — TRUNKS. One aimed at each home, then fillers spread through the
    //     angular gaps so no sector is left without a radial.
    let mut bearings: Vec<f64> = homes.iter().map(|h| bearing_of(*h - hub)).collect();
    let fillers = 3 + homes.len() / 2;
    for i in 0..fillers {
        bearings.push(std::f64::consts::TAU * (i as f64 + 0.5) / fillers as f64);
    }
    for (i, b) in bearings.iter().enumerate() {
        // AIMED at a home, not forced through it. Steering a trunk onto an exact
        // waypoint drags it off the systems in between and puts a kink where it
        // arrives; home connectivity is the spur pass's job instead (step 4), and
        // `every_home_reaches_the_network` is what holds that guarantee.
        let _ = i;
        // A sparse sector can leave a bearing with nothing in range at all, and a
        // radial that never forms is not a cosmetic loss: the hub-outward highway
        // is the shape the whole network is read against. So a starved bearing
        // reaches further and tries again before it is given up on.
        let ctrl = TRUNK_REACH_LADDER
            .iter()
            .find_map(|r| grow(&mut rng, hub, *b, anchors, spacing, radius, true, *r));
        if let Some(ctrl) = ctrl {
            lanes.push(finish(&mut next_id, ctrl, anchors, half_width, true, LaneKind::Trunk));
        }
    }

    // 2 — ARCS. Rings crossing the radials, so travel between two frontier
    //     systems does not have to go in to the hub and back out.
    for frac in [0.4, 0.7] {
        if let Some(ctrl) = arc(hub, radius * frac, anchors, spacing) {
            lanes.push(finish(&mut next_id, ctrl, anchors, half_width, false, LaneKind::Arc));
        }
    }

    // 3 — CHORDS. Criss-crossing routes seeded at a system's own singularity
    //     (secondary nucleation) rather than at the hub, grown BOTH ways. Added
    //     until the off-network floor is reached — target-driven, because a
    //     fixed count would leave a dense galaxy uniformly fast.
    for _ in 0..64 {
        if off_network_frac(&lanes, anchors, spacing) <= OFF_NETWORK_FLOOR {
            break;
        }
        let start = anchors[(rng.next_u64() as usize) % anchors.len().max(1)].pos;
        let bearing = rng.range(0.0, std::f64::consts::TAU);
        // Grow the BACK half first, then set the forward half off along the
        // heading the reversed back half arrives on. Two walks seeded nose to
        // tail on the same bearing each honour the turn cap against their OWN
        // start, and neither is answerable for the seam between them: the back
        // half can have drifted 35° a step for a dozen steps before the forward
        // half sets out on the original bearing, and the join inherits the whole
        // difference. That is a corner no growth rule ever agreed to — and being
        // in the grown polygon, it is one nothing downstream can repair either.
        let back = grow(&mut rng, start, bearing + std::f64::consts::PI, anchors, spacing, radius, false, 1.0);
        let seam = back
            .as_ref()
            .and_then(|b| b.get(1))
            .map_or(bearing, |second| bearing_of(start - *second));
        let fwd = grow(&mut rng, start, seam, anchors, spacing, radius, false, 1.0);
        let mut ctrl: Vec<Vec2> = Vec::new();
        if let Some(b) = back {
            ctrl.extend(b.into_iter().rev());
        }
        if let Some(f) = fwd {
            ctrl.extend(f.into_iter().skip(1));
        }
        if ctrl.len() >= 3 {
            lanes.push(finish(&mut next_id, ctrl, anchors, half_width, true, LaneKind::Chord));
        }
    }

    // 4 — SPURS. Home fairness is non-negotiable (lanes carry information as
    //     well as freight), but forcing a trunk to detour through a home puts a
    //     kink in it that fights the curvature and well constraints and drags the
    //     whole route off its other anchors. A short dedicated filament is
    //     cleaner, and the fiction already has one: the home's own singularity is
    //     a nucleation point, so a strand running to the nearest trunk is exactly
    //     what it would grow.
    let reach = spacing * 0.25;
    for h in homes {
        let nearest = lanes
            .iter()
            .filter_map(|l| l.nearest(*h).map(|(sm, d)| (sm.pos, d)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((join, d)) = nearest else { continue };
        if d <= reach {
            continue; // already served
        }
        let along = (join - *h).normalized();
        // A home-facing route terminates at the same explicit well-clearance
        // ring as every other centerline. The fleet covers the rest off-lane.
        let start = *h + along * WELL_STANDOFF;
        let ctrl = vec![start, start + (join - start) * 0.5, join];
        lanes.push(finish(&mut next_id, ctrl, anchors, half_width, false, LaneKind::Spur));
    }

    let mut net = LaneNetwork { lanes, ..Default::default() };
    net.rebuild_junctions();
    net
}

/// Grow a route outward from `from` along `bearing`, hopping anchor to anchor
/// under a heading cap. The cap is what shapes the route: hairpins are
/// structurally impossible rather than repaired afterwards, and the walk starves
/// naturally in the sparse frontier, which is where fade comes from.
#[allow(clippy::too_many_arguments)] // a growth walk genuinely has this many knobs
fn grow(
    rng: &mut Rng,
    from: Vec2,
    bearing: f64,
    anchors: &[LaneAnchor],
    spacing: f64,
    radius: f64,
    outward_only: bool,
    reach: f64,
) -> Option<Vec<Vec2>> {
    let mut ctrl = vec![from];
    let mut cur = from;
    let mut heading = bearing;
    // Reaching further is the SAFE way to rescue a starved walk. A corner's
    // radius is set by its SHORTER leg, so raising the maximum hop cannot tighten
    // anything — where widening the heading cone, the other obvious lever, bends
    // routes directly into the fastest hull's turning circle.
    let (lo, hi) = (spacing * STEP_MIN_FRAC, spacing * STEP_MAX_FRAC * reach);

    for _ in 0..12 {
        let mut best: Option<(f64, Vec2)> = None;
        for a in anchors {
            let d = cur.distance(a.pos);
            // The MINIMUM hop binds on every leg, the one out of the hub
            // included. Exempting that leg looks harmless — the hub is a
            // singularity, not a system, so system spacing has nothing to say
            // about it — but a corner's radius is set by its SHORTER leg, so a
            // short on-ramp puts an unflyable kink at the route's first system.
            if d < lo || d > hi {
                continue;
            }
            if ctrl.iter().any(|c| c.distance(a.pos) < lo * 0.5) {
                continue; // already on this route
            }
            let bear = bearing_of(a.pos - cur);
            let dev = ang_diff(bear, heading).abs();
            if dev > GROW_MAX_TURN_RAD {
                continue;
            }
            if outward_only && a.pos.length() < cur.length() {
                continue; // trunks run outward
            }
            // Prefer straight and near; a touch of noise so two seeds with the
            // same geometry do not produce the same route.
            let score = dev + (d / hi) * 0.4 + rng.range(0.0, 0.12);
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, a.pos));
            }
        }
        let Some((_, next)) = best else { break }; // fades into the frontier
        heading = bearing_of(next - cur);
        cur = next;
        ctrl.push(cur);
        if cur.length() > radius * 0.97 {
            break;
        }
    }
    (ctrl.len() >= 3).then_some(ctrl)
}

/// A ring of control points at `r`, pulled toward nearby anchors so the arc runs
/// past real systems rather than through empty space.
///
/// The pull is BOUNDED and VALIDATED, which the first cut of this was not. A ring
/// vertex free to jump to any anchor within `spacing * 0.8` can be yanked most of
/// a chord sideways, and two neighbouring vertices can chase anchors on opposite
/// flanks — that is how an arc on a 160k circle ended up with an 88° corner and a
/// 21k turning radius, tighter than any hull in the game can fly. A ring is
/// already navigable by construction (its spline radius tends to `r`); all the
/// damage came from the snap, so the snap is what gets a budget.
fn arc(hub: Vec2, r: f64, anchors: &[LaneAnchor], spacing: f64) -> Option<Vec<Vec2>> {
    let n = 10;
    let needed = TURN_K * fastest_lane_speed();

    // Build the ring at a given pull budget. No closing duplicate: an identical
    // first/last point collapses a knot interval and makes the spline degenerate
    // at the seam.
    let ring = |max_pull: f64| -> Vec<Vec2> {
        let mut ctrl: Vec<Vec2> = Vec::with_capacity(n);
        let mut taken: Vec<Vec2> = Vec::new();
        for i in 0..n {
            let th = std::f64::consts::TAU * i as f64 / n as f64;
            let ideal = hub + Vec2::from_polar(th, r);
            let snapped = anchors
                .iter()
                .map(|a| (a.pos, a.pos.distance(ideal)))
                .filter(|(p, d)| {
                    // Within budget, and not already claimed — two vertices
                    // sharing one anchor is a zero-length segment, hence a cusp.
                    *d < max_pull && !taken.iter().any(|t| t.distance(*p) < f64::EPSILON)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(p, _)| p);
            if let Some(p) = snapped {
                taken.push(p);
                ctrl.push(p);
            } else {
                ctrl.push(ideal);
            }
        }
        ctrl
    };

    // MEASURE, don't estimate. A closed-form pull budget has to model one vertex
    // moving in isolation, and that is not the situation: neighbouring vertices
    // chase different anchors, their pulls compound at the vertex between them,
    // and a pull shortens the adjacent segments as it bends them — which tightens
    // the radius twice over. Every seed's worst corner was an arc under exactly
    // that error. So back the pull off until the BAKED curve clears the bar.
    //
    // The margin absorbs the broad well-clearance relaxation `finish` applies
    // afterwards, which perturbs the ring again on the way to final geometry.
    let mut pull = spacing * 0.8;
    for _ in 0..10 {
        let ctrl = ring(pull);
        if route_radius(&ctrl) >= needed * ARC_CURVATURE_MARGIN {
            return (ctrl.len() >= 3).then_some(ctrl);
        }
        pull *= 0.6;
    }
    // A bare ring is navigable by construction — its spline radius tends to `r`,
    // which is a large fraction of the galaxy. Serving no anchors is a worse arc
    // but never an unflyable one.
    let ctrl = ring(0.0);
    (ctrl.len() >= 3).then_some(ctrl)
}

/// Bake the grown polygon into a ribbon and name it.
fn finish(
    next_id: &mut u32,
    ctrl: Vec<Vec2>,
    anchors: &[LaneAnchor],
    half_width: f64,
    tapers: bool,
    kind: LaneKind,
) -> Lane {
    let id = *next_id;
    *next_id += 1;
    // Growth establishes the topology and its broad turns. The final relaxation
    // then moves the baked centerline away from every well. Each displacement is
    // spread across several fastest-hull turning radii; this is why a 2.25k berth
    // does not introduce a 2.25k corner into a road a Scout must hold at speed.
    let (control, samples) = stand_off_from_wells(&ctrl, anchors);
    Lane {
        id,
        kind,
        name: route_name(id),
        samples,
        control,
        half_width,
        tapers,
    }
}

/// Relax a baked route away from gravity wells without tightening its turns.
///
/// The baked polyline is the mechanical centerline used by containment,
/// routing, rendering, and junction discovery. Working on that representation
/// makes the clearance guarantee continuous between samples instead of merely a
/// promise about sparse control vertices. A raised-cosine shoulder distributes
/// each correction over a broad arc; repeated projection closes the tiny error
/// left when several nearby wells push on the same stretch.
fn stand_off_from_wells(ctrl: &[Vec2], anchors: &[LaneAnchor]) -> (Vec<Vec2>, Vec<LaneSample>) {
    let baked = bake(ctrl);
    if baked.len() < 2 || anchors.is_empty() {
        return (ctrl.to_vec(), baked);
    }

    let mut points: Vec<Vec2> = baked.iter().map(|sample| sample.pos).collect();
    let shoulder = fastest_lane_speed() * TURN_K * 3.0;
    let target = WELL_STANDOFF + 8.0;
    let mut dodge_directions = vec![None; anchors.len()];

    for _ in 0..96 {
        let samples = samples_from_polyline(&points);
        let collisions: Vec<_> = anchors
            .iter()
            .enumerate()
            .filter_map(|(index, anchor)| {
                nearest_on_samples(&samples, anchor.pos)
                    .filter(|(_, _, d)| *d < target)
                    .map(|(on, tangent, d)| (index, anchor.pos, on, tangent, d))
            })
            .collect();
        if collisions.is_empty() {
            break;
        }

        let mut shifts = vec![Vec2::ZERO; points.len()];
        for (index, well, on, tangent, _distance) in collisions {
            // Always pass a well on the route's LEFT flank. Choosing whichever
            // side is initially nearest makes a string of anchor hits alternate
            // left/right, and their deliberately broad shoulders then cancel.
            let direction = *dodge_directions[index]
                .get_or_insert_with(|| Vec2::new(-tangent.y, tangent.x).normalized());
            let signed = (on.pos - well).dot(direction);
            let push = target - signed + 8.0;
            for (shift, sample) in shifts.iter_mut().zip(samples.iter()) {
                let along = (sample.s - on.s).abs();
                if along >= shoulder {
                    continue;
                }
                let phase = std::f64::consts::FRAC_PI_2 * along / shoulder;
                let weight = phase.cos().powi(2);
                *shift = *shift + direction * (push * weight);
            }
        }
        for (point, shift) in points.iter_mut().zip(shifts) {
            *point = *point + shift;
        }
    }

    let samples = samples_from_polyline(&points);
    if cfg!(debug_assertions) {
        let worst = anchors
            .iter()
            .enumerate()
            .filter_map(|(i, anchor)| {
                nearest_on_samples(&samples, anchor.pos).map(|(_, _, d)| (i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        debug_assert!(
            worst.is_none_or(|(_, d)| d + 1e-6 >= WELL_STANDOFF),
            "well relaxation stopped short: {worst:?}"
        );
    }

    // Every original Catmull control point occurs at this exact baked index.
    // Carry those relaxed positions forward so diagnostics and topology tests
    // describe the same centerline the sim flies.
    let control = (0..ctrl.len())
        .map(|i| points[(i * SAMPLES_PER_SEGMENT).min(points.len() - 1)])
        .collect();
    (control, samples)
}

fn samples_from_polyline(points: &[Vec2]) -> Vec<LaneSample> {
    let mut out = Vec::with_capacity(points.len());
    let mut s = 0.0;
    for (i, pos) in points.iter().copied().enumerate() {
        if i > 0 {
            s += points[i - 1].distance(pos);
        }
        let tangent = match (i.checked_sub(1), points.get(i + 1)) {
            (Some(prev), Some(next)) => (*next - points[prev]).normalized(),
            (_, Some(next)) => (*next - pos).normalized(),
            (Some(prev), None) => (pos - points[prev]).normalized(),
            _ => Vec2::ZERO,
        };
        out.push(LaneSample { pos, tangent, s });
    }
    out
}

/// Nearest point on the continuous polyline through `samples`.
fn nearest_on_samples(samples: &[LaneSample], p: Vec2) -> Option<(LaneSample, Vec2, f64)> {
    samples
        .windows(2)
        .filter_map(|w| {
            let d = w[1].pos - w[0].pos;
            let len_sq = d.length_sq();
            if len_sq <= 1e-12 {
                return None;
            }
            let t = ((p - w[0].pos).dot(d) / len_sq).clamp(0.0, 1.0);
            let pos = w[0].pos + d * t;
            let s = w[0].s + (w[1].s - w[0].s) * t;
            Some((LaneSample { pos, tangent: d.normalized(), s }, d.normalized(), p.distance(pos)))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

/// The fastest hull's lane speed — what every route must be navigable at. Kept
/// here rather than imported so `lane` stays free of the ship table; the value
/// is asserted against `ship::max_speed` by test.
fn fastest_lane_speed() -> f64 {
    115.0 * WARP_FACTOR * LANE_MULT
}

/// How wide a ribbon is, given the systems it threads.
///
/// No longer capped against a hull's turning circle. That cap existed to make
/// in-lane reversal geometrically impossible — keep the corridor narrower than
/// anything could turn inside of — and the argument did not survive contact:
/// past the alignment gate a hull's speed fell tenfold and its turning circle
/// with it, so a Titan came about in ~1,000 su inside a 5,900 su ribbon. §course-
/// change settles it as a RULE instead: you cannot steer above thrusters, so a
/// reversal costs a full shutdown and restart whatever the corridor's width.
fn ribbon_half_width(spacing: f64) -> f64 {
    spacing * LANE_WIDTH_FRAC * 0.5
}

/// Minimum curvature radius of a control polygon, sampled through its spline.
fn route_radius(pts: &[Vec2]) -> f64 {
    let s = bake(pts);
    s.windows(3)
        .map(|w| menger_radius(w[0].pos, w[1].pos, w[2].pos))
        .fold(f64::INFINITY, f64::min)
}

/// Share of anchors with no ribbon within a short hop.
fn off_network_frac(lanes: &[Lane], anchors: &[LaneAnchor], spacing: f64) -> f64 {
    if anchors.is_empty() {
        return 0.0;
    }
    let net = LaneNetwork { lanes: lanes.to_vec(), ..Default::default() };
    let off = anchors
        .iter()
        .filter(|a| {
            !lanes.iter().any(|l| {
                l.nearest(a.pos)
                    .is_some_and(|(_, d)| d <= l.half_width + spacing * SERVED_HOP_FRAC)
            }) && !net.on_lane(a.pos)
        })
        .count();
    off as f64 / anchors.len() as f64
}

pub fn median_spacing(anchors: &[LaneAnchor]) -> f64 {
    if anchors.len() < 2 {
        return 1.0;
    }
    let mut nearest: Vec<f64> = anchors
        .iter()
        .map(|a| {
            anchors
                .iter()
                .filter(|b| b.pos.distance(a.pos) > f64::EPSILON)
                .map(|b| b.pos.distance(a.pos))
                .fold(f64::INFINITY, f64::min)
        })
        .collect();
    nearest.sort_by(f64::total_cmp);
    nearest[nearest.len() / 2]
}

/// Bearing of a vector, in radians. (`Vec2` has `from_polar` but no inverse.)
fn bearing_of(v: Vec2) -> f64 {
    v.y.atan2(v.x)
}

fn ang_diff(a: f64, b: f64) -> f64 {
    let mut d = a - b;
    while d > std::f64::consts::PI {
        d -= std::f64::consts::TAU;
    }
    while d < -std::f64::consts::PI {
        d += std::f64::consts::TAU;
    }
    d
}

/// Route names read as ROUTES, not as places — a persistent name along the whole
/// trunk is what makes "take the Rip out and cut across" a sentence a player can
/// say.
fn route_name(id: u32) -> String {
    const TRUNK: [&str; 16] = [
        "The Rip", "Kepler Run", "The Long Reach", "Meridian Cut", "The Spindle",
        "Halloran Drift", "The Verge Road", "Cold Harbour Run", "The Seam",
        "Tanner's Reach", "The Gallows Road", "Windlass Run", "The Deep Furrow",
        "Ashfall Cut", "The Bright Lane", "Corvid Run",
    ];
    TRUNK[(id as usize) % TRUNK.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gateway(pos: Vec2) -> CommSite {
        CommSite { pos, throw: 40_000.0, gateway: true }
    }

    fn repeater(pos: Vec2) -> CommSite {
        CommSite { pos, throw: 80_000.0, gateway: false }
    }

    fn wide_gateway(pos: Vec2) -> CommSite {
        CommSite { pos, throw: 1_000_000.0, gateway: true }
    }

    fn signal_line(length: f64) -> LaneNetwork {
        let control = vec![Vec2::ZERO, Vec2::new(length * 0.5, 0.0), Vec2::new(length, 0.0)];
        LaneNetwork::of(vec![Lane {
            id: 77,
            kind: LaneKind::Trunk,
            name: "The Wire".into(),
            samples: bake_for_tests(&control),
            control,
            half_width: 2_000.0,
            tapers: false,
        }])
    }

    /// A galaxy shaped like the real generator's: area-uniform systems in an
    /// annulus, homes on a ring, hub at the origin.
    pub(super) fn galaxy(seed: u64, players: usize) -> (Vec2, Vec<LaneAnchor>, Vec<Vec2>, f64) {
        let radius = 4000.0 * (players as f64).sqrt() * WARP_FACTOR * 10.0;
        let count = 12 + 4 * players;
        let mut rng = Rng::new(seed);
        let anchors: Vec<LaneAnchor> = (0..count)
            .map(|_| {
                let t = rng.range(0.12 * 0.12, 0.96 * 0.96);
                let r = radius * t.sqrt();
                let th = rng.range(0.0, std::f64::consts::TAU);
                LaneAnchor { pos: Vec2::from_polar(th, r) }
            })
            .collect();
        let homes: Vec<Vec2> = (0..players)
            .map(|i| {
                Vec2::from_polar(std::f64::consts::TAU * i as f64 / players as f64, radius * 0.62)
            })
            .collect();
        // Homes ARE systems in the real generator, so they must be anchors here
        // too — a trunk can only be routed through a home that exists to route to.
        let mut anchors = anchors;
        anchors.extend(homes.iter().map(|p| LaneAnchor { pos: *p }));
        (Vec2::new(0.0, 0.0), anchors, homes, radius)
    }

    pub(super) fn net(seed: u64, players: usize) -> (LaneNetwork, Vec<LaneAnchor>, Vec<Vec2>, f64) {
        let (hub, anchors, homes, radius) = galaxy(seed, players);
        (generate(seed, hub, &anchors, &homes, radius), anchors, homes, radius)
    }

    /// DETERMINISM. The galaxy is generated from a seed and replayed
    /// byte-for-byte; a network that varied would break that guarantee at its
    /// root, since lanes decide travel, information delay and fog alike.
    #[test]
    fn generation_is_deterministic() {
        let (a, ..) = net(0xC0FFEE, 4);
        let (b, ..) = net(0xC0FFEE, 4);
        assert_eq!(a, b, "same seed, identical network");
        let (c, ..) = net(0xC0FFEF, 4);
        assert_ne!(a, c, "a different seed must produce a different network");
    }

    /// NAVIGABLE. A lane whose curve is tighter than the fastest hull's turning
    /// circle is a lane that hull physically cannot follow — a generation bug
    /// that would strand Scouts on the network's own highways.
    #[test]
    fn every_route_is_navigable_by_the_fastest_hull() {
        // ACROSS PLAYER COUNTS, not just four. Galaxy radius, system count and
        // median spacing all move with the roster, and generation budgets its
        // corners against spacing — so a table that only ever built a 4-player
        // map was blind by construction. A 2-player galaxy was shipping 8.4k
        // corners, a quarter of what a Scout can hold, the whole time it passed.
        for players in [2usize, 3, 4, 5, 6, 8] {
            for seed in [1u64, 2, 3, 7, 99, 2024, 12345] {
                let (n, ..) = net(seed, players);
                let scout_lane_speed = 115.0 * WARP_FACTOR * LANE_MULT;
                let needed = TURN_K * scout_lane_speed;
                let got = n.min_curvature_radius();
                assert!(
                    got >= needed,
                    "{players}p seed {seed}: min curvature {got:.0} < Scout's turning circle {needed:.0}",
                );
            }
        }
    }

    /// THE TURN CAP BINDS END TO END, seams included. A walk enforces its cap
    /// against its own heading, which says nothing about two walks joined back to
    /// back: a chord grows both ways from one system, and if the forward half sets
    /// off on the original bearing while the reversed back half arrives on a
    /// heading that drifted twelve steps away from it, the join inherits the whole
    /// difference. That produced 56° corners on a network capped at 35°.
    ///
    /// Worth checking even though the corner is survivable — `finish` will rescue
    /// an unflyable route by retreating it toward the grown line, so a seam like
    /// this does not show up as a curvature failure. It shows up as routes quietly
    /// giving back the clearance and shape they were relaxed into.
    ///
    /// Arcs are exempt: a ring turns `TAU / n` at every vertex by construction.
    #[test]
    fn the_turn_cap_holds_across_a_seam() {
        // Room for the well offsets, which nudge control points after growth.
        let ceiling = GROW_MAX_TURN_RAD * 1.25;
        // A seam only bites when two long walks meet at a wide angle, which is
        // rare enough that a handful of seeds will not find one — the first cut of
        // this test swept twelve and saw nothing worse than the cap itself.
        for players in [2usize, 3, 4, 6] {
            for seed in 0u64..24 {
                let (n, ..) = net(seed, players);
                for l in n.lanes.iter().filter(|l| l.kind != LaneKind::Arc) {
                    let worst = l
                        .control
                        .windows(3)
                        .map(|w| ang_diff(bearing_of(w[2] - w[1]), bearing_of(w[1] - w[0])).abs())
                        .fold(0.0f64, f64::max);
                    assert!(
                        worst <= ceiling,
                        "{players}p seed {seed}: {} turns {:.0}° on a network capped at {:.0}°",
                        l.name,
                        worst.to_degrees(),
                        GROW_MAX_TURN_RAD.to_degrees(),
                    );
                }
            }
        }
    }

    /// RADIALS MUST ACTUALLY GROW. The hub-outward highway is the shape the whole
    /// network is read against, and it is the one piece of the topology that no
    /// other pass can stand in for — arcs run crosswise, chords wander, spurs are
    /// one home long. Nothing used to check that any of them survived generation,
    /// and for a long stretch almost none did: seven of nine bearings starved on
    /// their FIRST step, because the walk demanded a system within a band that,
    /// for an area-uniform galaxy, is nearly empty near the hub.
    #[test]
    fn radials_grow_from_the_hub_and_reach_the_rim() {
        for players in [3usize, 4, 6] {
            let bearings = players + 3 + players / 2;
            let mut total = 0.0;
            let seeds = [1u64, 2, 3, 7, 99, 2024, 12345];
            for seed in seeds {
                let (n, _, _, radius) = net(seed, players);
                let trunks: Vec<&Lane> =
                    n.lanes.iter().filter(|l| l.kind == LaneKind::Trunk).collect();
                assert!(
                    !trunks.is_empty(),
                    "{players}p seed {seed}: not one radial survived {bearings} bearings",
                );
                let reach = trunks
                    .iter()
                    .map(|l| l.control.last().unwrap().length() / radius)
                    .fold(0.0f64, f64::max);
                assert!(
                    reach >= 0.7,
                    "{players}p seed {seed}: the longest radial dies {:.0}% out — a stub, not a highway",
                    reach * 100.0,
                );
                total += trunks.len() as f64;
            }
            // Aggregate, so a lucky seed cannot carry a starving generator.
            let mean = total / seeds.len() as f64;
            assert!(
                mean >= bearings as f64 * 0.45,
                "{players}p: only {mean:.1} of {bearings} bearings grow a radial on average",
            );
        }
    }



    /// HOME FAIRNESS. Lanes are the information network as well as the logistics
    /// one, so a player whose home is off-network plays a materially different
    /// game. Every home gets access — this is a Pillar 1 requirement, not polish.
    #[test]
    fn every_home_reaches_the_network() {
        for seed in [11u64, 22, 33, 44] {
            let (n, anchors, homes, _) = net(seed, 4);
            // Stated as a fraction of a HOP, which is what "a short run to the
            // highway" actually means, rather than an arbitrary slice of radius.
            let reach = median_spacing(&anchors) * 0.25;
            for (i, h) in homes.iter().enumerate() {
                let best = n
                    .lanes
                    .iter()
                    .filter_map(|l| l.nearest(*h).map(|(_, d)| d))
                    .fold(f64::INFINITY, f64::min);
                assert!(best <= reach, "seed {seed}: home {i} is {best:.0} from any lane (max {reach:.0})");
            }
        }
    }

    /// GRAVITY WELLS ARE HOLES IN THE ROAD NETWORK. The centerline must keep
    /// its full berth continuously, including between baked vertices and from
    /// systems that were not selected as this route's own control points.
    #[test]
    fn every_lane_keeps_its_standoff_from_every_well() {
        for players in [2usize, 4, 6] {
            for seed in [1u64, 2, 7, 99, 2024] {
                let (n, anchors, ..) = net(seed, players);
                for lane in &n.lanes {
                    for (well, anchor) in anchors.iter().enumerate() {
                        let clearance = lane.nearest(anchor.pos).unwrap().1;
                        assert!(
                            clearance + 1e-6 >= WELL_STANDOFF,
                            "{players}p seed {seed}: {} passes well {well} at {clearance:.2} su (< {WELL_STANDOFF:.2})",
                            lane.name,
                        );
                    }
                }
            }
        }
    }

    /// OFF-NETWORK FLOOR. If chords keep being added until the map is uniformly
    /// fast, "off the highway" stops being a strategic category and the whole
    /// information topology loses its meaning. Generation is target-driven for
    /// exactly this reason, and the target is a post-condition of the pipeline.
    #[test]
    fn a_real_share_of_systems_stays_off_network() {
        for seed in [8u64, 9, 10] {
            let (n, anchors, ..) = net(seed, 4);
            let off = off_network_frac(&n.lanes, &anchors, median_spacing(&anchors));
            assert!(off >= 0.15, "seed {seed}: only {off:.0}% off-network — the map is uniformly fast");
            assert!(off <= 0.70, "seed {seed}: {off:.0}% off-network — the network barely exists");
        }
    }

    /// NON-ACCUMULATION. Strain is a property of the substrate at a point, not a
    /// stack of bonuses: two panes of glass do not double the refractive index.
    /// Constructed overlap, asserted directly, because this is the one rule an
    /// implementer would naturally get wrong by summing.
    #[test]
    fn overlapping_lanes_do_not_accumulate() {
        let straight = |id: u32, pts: Vec<Vec2>| Lane {
            id,
            kind: LaneKind::Trunk,
            name: format!("L{id}"),
            samples: bake(&pts),
            control: pts,
            half_width: 500.0,
            tapers: false,
        };
        // Two routes crossing at the origin, plus a third nearly co-linear with
        // the first — so the point is covered three times over.
        let n = LaneNetwork::of(vec![
                straight(0, vec![Vec2::new(-9000.0, 0.0), Vec2::ZERO, Vec2::new(9000.0, 0.0)]),
                straight(1, vec![Vec2::new(0.0, -9000.0), Vec2::ZERO, Vec2::new(0.0, 9000.0)]),
                straight(2, vec![Vec2::new(-9000.0, 60.0), Vec2::new(0.0, 60.0), Vec2::new(9000.0, 60.0)]),
        ]);
        let east = Vec2::new(1.0, 0.0);
        assert_eq!(n.hits(Vec2::ZERO).len(), 3, "the point really is inside three ribbons");
        assert_eq!(
            n.speed_factor(Vec2::ZERO, east),
            LANE_MULT,
            "three overlapping lanes still yield exactly one lane's benefit",
        );
    }

    /// THE ALIGNMENT GATE. A fleet cutting ACROSS a lane earns nothing — which is
    /// what makes the bands mean something, stops a lane being a free boost for
    /// crossing traffic, and makes a turn cost its whole arc.
    #[test]
    fn the_benefit_is_alignment_gated_and_direction_agnostic() {
        let l = Lane {
            id: 0,
            kind: LaneKind::Trunk,
            name: "L".into(),
            samples: bake(&[Vec2::new(-9000.0, 0.0), Vec2::ZERO, Vec2::new(9000.0, 0.0)]),
            control: vec![Vec2::new(-9000.0, 0.0), Vec2::ZERO, Vec2::new(9000.0, 0.0)],
            half_width: 500.0,
            tapers: false,
        };
        let n = LaneNetwork::of(vec![l]);
        let p = Vec2::ZERO;
        assert_eq!(n.speed_factor(p, Vec2::new(1.0, 0.0)), LANE_MULT, "along the route: full benefit");
        assert_eq!(
            n.speed_factor(p, Vec2::new(-1.0, 0.0)),
            LANE_MULT,
            "and the SAME the other way — a lane is not a current, both directions are equally fast",
        );
        assert_eq!(n.speed_factor(p, Vec2::new(0.0, 1.0)), 1.0, "crossing it earns nothing");
        assert_eq!(n.speed_factor(p, Vec2::ZERO), 1.0, "and a stationary fleet has no heading to align");
        assert_eq!(
            n.speed_factor(Vec2::new(0.0, 4000.0), Vec2::new(1.0, 0.0)),
            1.0,
            "outside the ribbon, nothing",
        );
    }

    /// THE THREE REGIMES. Hyperspace is an overlay: the same fleet at the same
    /// point is in one of three speed regimes depending only on its drive and
    /// its alignment. Drive off is the pre-hyperspace game, exactly.
    #[test]
    fn the_three_regimes_resolve_from_drive_and_alignment() {
        let l = Lane {
            id: 0,
            kind: LaneKind::Trunk,
            name: "L".into(),
            samples: bake(&[Vec2::new(-90_000.0, 0.0), Vec2::ZERO, Vec2::new(90_000.0, 0.0)]),
            control: vec![Vec2::new(-90_000.0, 0.0), Vec2::ZERO, Vec2::new(90_000.0, 0.0)],
            half_width: 800.0,
            tapers: false,
        };
        let net = LaneNetwork::of(vec![l]);
        let env = TransitEnv { lanes: &net, wells: &[] };
        let on = Vec2::ZERO;
        let off = Vec2::new(0.0, 40_000.0);
        let east = Vec2::new(1.0, 0.0);

        assert_eq!(env.factor(on, east, false), 1.0, "drive off: normal space, wherever you are");
        assert_eq!(env.factor(off, east, true), WARP_FACTOR, "drive on, off-lane: warp");
        assert_eq!(
            env.factor(on, east, true),
            WARP_FACTOR * LANE_MULT,
            "drive on, aligned on a lane: the highway",
        );
        assert_eq!(
            env.factor(on, Vec2::new(0.0, 1.0), true),
            WARP_FACTOR,
            "crossing the lane earns warp speed only — the alignment gate",
        );
    }

    /// NO DRIVE IN A GRAVITY WELL. One rule, and it is what keeps blockade,
    /// siege, platforms, docking and the ground war working untouched — and what
    /// stops a besieged fleet hyperspacing out from under an investment.
    #[test]
    fn the_drive_will_not_light_inside_a_gravity_well() {
        let net = LaneNetwork::default();
        let star = Vec2::new(5_000.0, 0.0);
        let wells = [(star, HYPERLIMIT)];
        let env = TransitEnv { lanes: &net, wells: &wells };
        let east = Vec2::new(1.0, 0.0);

        assert_eq!(env.factor(star, east, true), 1.0, "at the star: normal space, drive or no drive");
        assert_eq!(
            env.factor(star + Vec2::new(HYPERLIMIT * 0.9, 0.0), east, true),
            1.0,
            "still inside the well",
        );
        assert_eq!(
            env.factor(star + Vec2::new(HYPERLIMIT * 1.1, 0.0), east, true),
            WARP_FACTOR,
            "clear of the well, the drive lights",
        );
    }

    /// THE HEADLINE OF THE MILESTONE: a fleet at lane speed CANNOT come about
    /// inside the ribbon. Not because a rule forbids it — because its turning
    /// circle is wider than the lane, so the attempt carries it out into open
    /// hyperspace. Reversal costs an exit and an arc, as geometry.
    #[test]
    fn a_fleet_cannot_come_about_inside_a_lane() {
        use crate::movement::advance_turning;
        let half_width = 800.0;
        // Running east down the lane at Convoy lane speed, ordered to reverse.
        let speed = 40.0 * WARP_FACTOR * LANE_MULT;
        let radius = TransitEnv::turn_radius(speed);
        assert!(radius > half_width, "the premise: circle {radius:.0} exceeds half-width {half_width:.0}");

        let mut pos = Vec2::ZERO;
        let mut vel = Vec2::new(speed, 0.0);
        let dest = Vec2::new(-500_000.0, 0.0); // straight back the way it came
        let mut left_the_ribbon = false;
        for _ in 0..600 {
            let step = advance_turning(pos, vel, dest, speed, crate::config::DT, radius);
            pos = step.pos;
            vel = step.vel;
            if pos.y.abs() > half_width {
                left_the_ribbon = true;
                break;
            }
            // If it ever gets pointed back west while still inside, the premise fails.
            assert!(
                !(vel.x < 0.0 && pos.y.abs() <= half_width),
                "it came about INSIDE the ribbon at y={:.0}",
                pos.y,
            );
        }
        assert!(left_the_ribbon, "the turn must carry it out of the lane");
    }

    /// The arc costs every hull the SAME time — `π·k` — because radius scales
    /// with speed and the time cancels. The lateral exit is what makes reversal
    /// hurt more for heavy hulls, not the turn itself.
    #[test]
    fn the_turn_costs_every_hull_the_same_seconds() {
        use crate::movement::advance_turning;
        let secs = |base: f64| {
            let speed = base * WARP_FACTOR * LANE_MULT;
            let radius = TransitEnv::turn_radius(speed);
            let mut pos = Vec2::ZERO;
            let mut vel = Vec2::new(speed, 0.0);
            let dest = Vec2::new(-1.0e9, 0.0);
            let mut t = 0.0;
            for _ in 0..100_000 {
                let step = advance_turning(pos, vel, dest, speed, crate::config::DT, radius);
                pos = step.pos;
                vel = step.vel;
                t += crate::config::DT;
                if vel.x < 0.0 && vel.y.abs() < speed * 0.02 {
                    break; // come about
                }
            }
            t
        };
        let expected = std::f64::consts::PI * TURN_K;
        for base in [23.0, 40.0, 115.0] {
            let got = secs(base);
            assert!(
                (got - expected).abs() < expected * 0.15,
                "hull at {base}: turn took {got:.1}s, expected ~{expected:.1}s",
            );
        }
    }

    /// THE GUARANTEE THE WHOLE INFORMATION MODEL RESTS ON: a signal beats a ship
    /// on ANY route the ship can fly, in every regime, by exactly `c / v`.
    ///
    /// It holds because strain scales signals and hulls together — which is why
    /// `c` never had to be rescaled to accommodate a 10× lane, and why orders can
    /// always catch a fleet. If this ever fails, commanding a distant fleet
    /// becomes impossible and §6.2's order lifecycle has no meaning.
    #[test]
    fn a_signal_outruns_every_hull_in_every_regime() {
        let c = 400.0;
        let fastest = 115.0; // Scout
        for (regime, sig, hull) in [
            ("normal", c, fastest),
            ("warp", c * WARP_FACTOR, fastest * WARP_FACTOR),
            (
                "lane",
                c * WARP_FACTOR * LANE_MULT,
                fastest * WARP_FACTOR * LANE_MULT,
            ),
        ] {
            let ratio = sig / hull;
            assert!(ratio >= 2.0, "{regime}: signal only {ratio:.2}× the fastest hull");
            assert!(
                (ratio - c / fastest).abs() < 1e-9,
                "{regime}: the ratio must be IDENTICAL to normal space ({:.2}), got {ratio:.2}",
                c / fastest,
            );
        }
    }

    /// LANE PROXIMITY IS WORTH `LANE_MULT` ON INFORMATION. This is the property
    /// that makes the lane network the information network as well as the
    /// logistics one — core worlds responsive, off-network colonies genuinely
    /// remote — and it is what gives "lane control is intel control" its teeth.
    #[test]
    fn riding_a_lane_is_ten_times_faster_for_information() {
        let c = 400.0;
        let span = 200_000.0;
        let lane = Lane {
            id: 0,
            kind: LaneKind::Trunk,
            name: "Trunk".into(),
            samples: bake(&[Vec2::ZERO, Vec2::new(span * 0.5, 0.0), Vec2::new(span, 0.0)]),
            control: vec![Vec2::ZERO, Vec2::new(span * 0.5, 0.0), Vec2::new(span, 0.0)],
            half_width: 800.0,
            tapers: false,
        };
        let net = LaneNetwork::of(vec![lane]);
        let (a, b) = (Vec2::ZERO, Vec2::new(span, 0.0));

        let on_lane = net.delay(a, b, c);
        let off_lane = LaneNetwork::default().delay(a, b, c);
        assert!(
            (off_lane / on_lane - LANE_MULT).abs() < 0.05 * LANE_MULT,
            "on the trunk {on_lane:.1}s vs warp {off_lane:.1}s — expected ~{LANE_MULT}×",
        );

        // A world set back from the trunk pays the hop, and no more.
        let aside = Vec2::new(span, 20_000.0);
        let hop = net.delay(a, aside, c);
        assert!(hop > on_lane, "being off the highway costs something");
        assert!(hop < off_lane, "...but far less than being off-network entirely");
    }

    /// THE PLAYTEST TARGET, pinned: hub → rim is ~20 s off-lane.
    ///
    /// This is the number the whole information model is felt through, and it is
    /// the reason the galaxy scales with `H` at all — signals ride hyperspace, so
    /// without the scaling this crossing collapses to 4 s (and 0.4 s on a lane,
    /// which at 10 Hz updates is four frames and reads as instant). If a future
    /// tuning pass moves `H` or `c` without moving the galaxy, this fails loudly.
    #[test]
    fn hub_to_rim_stays_about_twenty_seconds_off_lane() {
        let secs = |players: u32| {
            let cfg = crate::config::SimConfig::for_players(1, players);
            // Off-lane hyperspace is the typical crossing: signals ride the
            // overlay everywhere, and earn the extra 10× only inside a ribbon.
            // The FASTEST route: riding a trunk, which is how quickly news can
            // possibly cross the map.
            cfg.galaxy_radius / (cfg.c * WARP_FACTOR * LANE_MULT)
        };

        // THE PLAYTEST BASELINE, at the default player count.
        let base = secs(4);
        assert!((base - 20.0).abs() < 1.0, "4p hub→rim ON A TRUNK is {base:.1}s, target ~20s");

        // Bigger galaxies take proportionally longer, exactly as they always
        // have — radius grows as √players so the dark between homes stays
        // proportional. The rescale must not have disturbed that law.
        for players in [8u32, 12] {
            let expect = base * (players as f64 / 4.0).sqrt();
            let got = secs(players);
            assert!(
                (got - expect).abs() < 0.5,
                "{players}p: {got:.1}s, expected {expect:.1}s by the √players law",
            );
        }

        // OFF the network it must be MUCH slower — that spread is what makes the
        // lane network the information network rather than a mild convenience.
        let cfg = crate::config::SimConfig::for_players(1, 4);
        let off_lane = cfg.galaxy_radius / (cfg.c * WARP_FACTOR);
        assert!(
            (off_lane / base - LANE_MULT).abs() < 0.01,
            "off-lane should be exactly {LANE_MULT}× slower than a trunk, got {off_lane:.0}s vs {base:.0}s",
        );
        // ...and in normal space, slower again by the hyperspace factor.
        let sublight = cfg.galaxy_radius / cfg.c;
        assert!(
            (sublight / off_lane - WARP_FACTOR).abs() < 0.01,
            "normal space should be {WARP_FACTOR}× slower again, got {sublight:.0}s",
        );
    }

    /// A ROUTE FOLLOWS THE CURVE. Lanes are splines, so riding one means flying
    /// its centerline — a straight line at the destination would clip ribbons at
    /// useless angles and earn nearly nothing. This is the piece that makes the
    /// speed benefit actually reachable rather than incidental.
    #[test]
    fn a_route_bends_along_the_lane_instead_of_flying_straight() {
        // A lane that bows well off the direct line between two points.
        let ctrl = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100_000.0, 60_000.0),
            Vec2::new(200_000.0, 0.0),
        ];
        let net = LaneNetwork::of(vec![Lane {
                id: 0,
                kind: LaneKind::Trunk,
                name: "Bow".into(),
                samples: bake(&ctrl),
                control: ctrl,
                half_width: 2_000.0,
                tapers: false,
        }]);
        let (from, to) = (Vec2::new(0.0, 0.0), Vec2::new(200_000.0, 0.0));
        let (v_deep, v_lane) = (200.0, 2_000.0);

        let route = net.route(from, to, v_deep, v_lane);
        assert!(route.len() > 2, "a lane route is a path, not a single hop");
        assert_eq!(route.last().unwrap().to, to, "and it always ends where it was sent");

        // It genuinely BOWS: the straight line is y=0 throughout, so any real
        // deviation proves the fleet is following the curve rather than the chord.
        let peak = route.iter().map(|l| l.to.y.abs()).fold(0.0, f64::max);
        assert!(peak > 20_000.0, "the route hugs the lane's bow (peak y = {peak:.0})");

        // And it is chosen because it is FASTER despite being longer.
        let path_len: f64 = std::iter::once(from)
            .chain(route.iter().map(|l| l.to))
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| w[0].distance(w[1]))
            .sum();
        assert!(path_len > from.distance(to), "the lane route is physically longer");
        assert!(
            path_len / v_lane < from.distance(to) / v_deep,
            "...and still quicker, which is the whole reason to take it",
        );
    }

    /// §junction: A LONG LEG CHANGES ROUTES RATHER THAN RIDING ONE AND WALKING.
    ///
    /// The old planner tried each lane alone, so a path was always "hop out to
    /// one route, ride it, hop to the destination". Riding is nearly free, which
    /// means the trip was priced almost entirely by those two hops — measured on
    /// one leg, a third of the distance and eighty-three percent of the clock —
    /// and no amount of picking a better single lane could shorten them. The fix
    /// is to change lanes where two of them cross.
    #[test]
    fn a_long_route_changes_lanes_where_they_cross() {
        let (n, anchors, ..) = net(1, 4);
        assert!(!n.junctions.is_empty(), "a connected network has crossings");
        let from = anchors[0].pos;
        let to = anchors
            .iter()
            .max_by(|a, b| a.pos.distance(from).total_cmp(&b.pos.distance(from)))
            .unwrap()
            .pos;
        let route = n.route(from, to, 500.0, 5_000.0);
        assert!(route.len() > 2, "a long leg should route through the network");

        // Assert on the PATH, not on which ribbons the waypoints happen to sit
        // in. Ribbons overlap at a crossing by definition, so waypoint membership
        // says nothing about whether the path took one — the first version of
        // this test passed with transfers disabled entirely.
        let path = n
            .best_path(from, to, LANE_MULT, TURN_K * 500.0)
            .expect("a long leg should find a path through the network");
        let lanes: Vec<usize> = path.iter().map(|(li, _)| *li).collect();
        let distinct: std::collections::BTreeSet<usize> = lanes.iter().copied().collect();
        assert!(
            distinct.len() >= 2,
            "the leg rode lane(s) {distinct:?} end to end — the crossings are not being used",
        );
    }

    /// §junction: THE NETWORK IS DERIVED, SO IT MUST BE DERIVED THE SAME WAY TWICE.
    /// Junctions ride in the snapshot; if two builds of one seed disagreed, a
    /// reloaded galaxy would route differently from the one that was saved.
    #[test]
    fn the_junction_graph_is_deterministic() {
        let (a, ..) = net(0x5EED, 4);
        let (b, ..) = net(0x5EED, 4);
        assert_eq!(a.junctions, b.junctions);
        assert_eq!(a.stops, b.stops);
    }

    /// A crossing BETWEEN baked vertices is still a junction. The old detector
    /// compared only the four endpoints in this fixture, found them all far
    /// apart, and omitted the obvious corner at the origin. On an arc that makes
    /// the route continue almost a full revolution before another junction lets
    /// it change lanes.
    #[test]
    fn a_crossing_between_samples_creates_the_corner_it_visibly_has() {
        let lane = |id: u32, a: Vec2, b: Vec2| {
            let tangent = (b - a).normalized();
            Lane {
                id,
                kind: LaneKind::Trunk,
                name: format!("Sparse {id}"),
                control: vec![a, b],
                samples: vec![
                    LaneSample { pos: a, tangent, s: 0.0 },
                    LaneSample { pos: b, tangent, s: a.distance(b) },
                ],
                half_width: 10.0,
                tapers: false,
            }
        };
        let n = LaneNetwork::of(vec![
            lane(0, Vec2::new(-1_000.0, 0.0), Vec2::new(1_000.0, 0.0)),
            lane(1, Vec2::new(0.0, -1_000.0), Vec2::new(0.0, 1_000.0)),
        ]);
        assert_eq!(n.junctions.len(), 1, "the continuous centerlines cross once");
        assert!(
            n.junctions[0].pos.distance(Vec2::ZERO) < 1e-9,
            "the handoff belongs at the actual crossing, got {:?}",
            n.junctions[0].pos,
        );

        let route = n.route(
            Vec2::new(-900.0, 0.0),
            Vec2::new(0.0, 900.0),
            1.0,
            100.0,
        );
        let used: std::collections::BTreeSet<u32> =
            route.iter().filter_map(|leg| leg.lane).collect();
        assert_eq!(used, [0, 1].into_iter().collect(), "the route turns at that crossing");
        let corner = route
            .windows(2)
            .find(|w| w[0].lane != w[1].lane)
            .expect("the path changes lanes");
        assert!(
            corner[0].to.distance(Vec2::ZERO) < 1e-9
                && corner[1].to.distance(Vec2::ZERO) < 1e-9,
            "both centerlines hand off at the same proper corner",
        );
    }

    /// When no lane helps, the route is EMPTY — the common case near home, and
    /// the fallback that keeps every pre-lane behaviour intact.
    ///
    /// Empty rather than `[to]`: "no route" and "a route with one waypoint that
    /// happens to be the destination" are the same flight, and `advance` already
    /// steers at the order's destination when there is nothing to follow. Saying
    /// it once is worth more than encoding it twice.
    #[test]
    fn a_route_with_no_useful_lane_is_a_straight_run() {
        let net = LaneNetwork::default();
        let (from, to) = (Vec2::new(0.0, 0.0), Vec2::new(50_000.0, 0.0));
        assert!(net.route(from, to, 200.0, 2_000.0).is_empty());
        // And with a network that simply does not go that way.
        let (n, anchors, ..) = super::tests::net(11, 4);
        let a = anchors[0].pos;
        assert!(n.route(a, a + Vec2::new(300.0, 0.0), 200.0, 2_000.0).is_empty(), "a hop next door needs no lane");
    }

    /// §buoys: A CORPORATION WITH NO COMM SITES has no hyperspace entry point.
    /// Its command center is an endpoint, not a relay, so every order travels at
    /// plain warp until physical infrastructure is present.
    #[test]
    fn a_corp_with_no_comm_sites_sends_every_order_at_plain_warp() {
        let ctrl = vec![Vec2::new(0.0, 0.0), Vec2::new(100_000.0, 0.0)];
        let n = LaneNetwork::of(vec![Lane {
            id: 0,
            kind: LaneKind::Trunk,
            name: "Home lane".into(),
            samples: bake_for_tests(&ctrl),
            control: ctrl,
            half_width: 2_000.0,
            tapers: false,
        }]);
        let home = Vec2::new(10_000.0, 0.0);
        let raider = Vec2::new(90_000.0, 30_000.0);
        let c = 400.0;
        let warp_time = home.distance(raider) / (c * WARP_FACTOR);
        assert!(
            (n.signal(home, raider, c, &[]).0 - warp_time).abs() < 1e-6,
            "no relay anywhere is plain warp light"
        );
        let (home_only, hops) = n.signal(home, raider, c, &[]);
        assert!((home_only - warp_time).abs() < 1e-6);
        assert_eq!(hops.len(), 1, "the order takes one straight hop");
        assert_eq!(hops[0].to, raider);
        assert_eq!(hops[0].lane, None, "the no-site order claims no lane");
    }

    /// §comms-infra: two GATEWAYS with overlapping coverage can board, carry,
    /// and discharge a signal on the same lane.
    #[test]
    fn two_covered_gateways_on_one_lane_carry_the_signal_at_lane_speed() {
        let (n, ..) = net(1, 4);
        let l = &n.lanes[0];
        let (a, b) = (l.at(l.length() * 0.1), l.at(l.length() * 0.9));
        let c = 400.0;
        let warp_only = n.signal(a, b, c, &[]).0;
        let (relayed, hops) = n.signal(a, b, c, &[wide_gateway(a), wide_gateway(b)]);
        assert!(
            relayed < warp_only * 0.5,
            "two covered gateways on one lane should be far quicker ({relayed:.1}s vs {warp_only:.1}s)",
        );
        assert!(hops.iter().any(|h| h.lane == Some(l.id)), "and the path says which road it rode");
        assert_eq!(hops.last().unwrap().to, b, "a signal always ends where it was sent");
    }

    /// §buoys: THE HOPS CARRY THEIR CLOCK. Each hop's `t` is the cumulative
    /// arrival time straight from the relay search, so the order comet can
    /// replay the journey exactly — fast along the lane, slow across the gaps
    /// — without re-deriving any speed. Monotonic, and the last hop lands at
    /// precisely the total the field quotes.
    #[test]
    fn signal_hops_carry_cumulative_times_that_land_on_the_total() {
        let (n, ..) = net(1, 4);
        let l = &n.lanes[0];
        let (a, b) = (l.at(l.length() * 0.1), l.at(l.length() * 0.9));
        let c = 400.0;
        let (total, hops) = n.signal(a, b, c, &[wide_gateway(a), wide_gateway(b)]);
        assert!(hops.len() >= 2, "a relayed journey has real hops");
        let mut prev = 0.0;
        for h in &hops {
            assert!(h.t > prev - 1e-9, "hop times run forward ({:.2} after {prev:.2})", h.t);
            prev = h.t;
        }
        assert!(
            (hops.last().unwrap().t - total).abs() < 1e-6,
            "the last hop arrives exactly at the quoted total ({:.3} vs {total:.3})",
            hops.last().unwrap().t
        );
        // And the unrelayed fallback stamps its single hop with the whole trip.
        let (direct_t, direct_hops) = n.signal(a, b, c, &[]);
        assert_eq!(direct_hops.len(), 1);
        assert!((direct_hops[0].t - direct_t).abs() < 1e-9);
    }

    /// The order comet must trace the CURVE whose arc length supplied its fast
    /// arrival time. Relay and endpoint nodes alone draw the chord between them,
    /// making a lane-speed order look as though it crossed open space in a
    /// straight line.
    #[test]
    fn a_coupled_order_path_contains_the_lane_curve_not_just_its_endpoints() {
        let control = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100_000.0, 60_000.0),
            Vec2::new(200_000.0, 0.0),
        ];
        let n = LaneNetwork::of(vec![Lane {
            id: 42,
            kind: LaneKind::Trunk,
            name: "Signal Bow".into(),
            samples: bake_for_tests(&control),
            control,
            half_width: 2_000.0,
            tapers: false,
        }]);
        let lane = &n.lanes[0];
        let home = lane.at(lane.length() * 0.05);
        let buoy = lane.at(lane.length() * 0.35);
        let hull = lane.at(lane.length() * 0.95);
        let buoys = [wide_gateway(home), wide_gateway(buoy)];
        let field = DelayField { lanes: &n, sites: &buoys, c: 400.0 };

        let coarse = n.signal_to_coupled(home, hull, field.c, &buoys).1;
        let traced = field.path_to_coupled(home, hull);
        assert!(
            traced.len() > coarse.len() + 3,
            "the drawable path should gain centerline samples ({} coarse, {} traced)",
            coarse.len(),
            traced.len(),
        );
        let peak = traced.iter().map(|hop| hop.to.y).fold(f64::NEG_INFINITY, f64::max);
        assert!(
            peak > 40_000.0,
            "the path should follow the lane's 60k-su bow instead of its near-horizontal chord (peak {peak:.0})",
        );
        let mut previous = 0.0;
        for hop in &traced {
            assert!(hop.t >= previous - 1e-9, "expanded hop times must remain monotonic");
            previous = hop.t;
        }
        assert!(
            (traced.last().unwrap().t - field.to_coupled(home, hull)).abs() < 1e-9,
            "expanding the curve must not alter the authoritative arrival time",
        );
    }

    /// Repeaters provide coverage but are never boarding or exit points. Their
    /// positions disappear from a same-lane drawable ride between gateways.
    #[test]
    fn wire_carries_the_ride_but_only_a_gateway_boards_it() {
        let n = signal_line(400_000.0);
        let (entry, r1, r2, exit) = (
            Vec2::ZERO,
            Vec2::new(110_000.0, 0.0),
            Vec2::new(220_000.0, 0.0),
            Vec2::new(300_000.0, 0.0),
        );
        let c = 400.0;
        let bare = n.signal(entry, exit, c, &[gateway(entry), gateway(exit)]).0;
        let sites = [gateway(entry), repeater(r1), repeater(r2), gateway(exit)];
        let (wired, hops) = n.signal(entry, exit, c, &sites);
        assert!(wired < bare * 0.5, "wire should turn the span into a lane ride");
        assert!(hops.iter().any(|hop| hop.lane == Some(77)));
        assert!(
            hops.iter().filter(|hop| hop.lane.is_some()).all(|hop| hop.to != r1 && hop.to != r2),
            "repeaters carry coverage without becoming drawable exits"
        );

        let (from_repeater, hops) =
            n.signal(r1, exit, c, &[repeater(r1), gateway(exit)]);
        let warp = r1.distance(exit) / (c * WARP_FACTOR);
        assert!((from_repeater - warp).abs() < 1e-9);
        assert!(hops.iter().all(|hop| hop.lane.is_none()), "nothing boards at wire");

        // Coverage gaps are measured across junctions as one continuous arc.
        let lane = |id: u32, control: Vec<Vec2>| Lane {
            id,
            kind: LaneKind::Trunk,
            name: format!("Wire {id}"),
            samples: bake_for_tests(&control),
            control,
            half_width: 2_000.0,
            tapers: false,
        };
        let crossing = LaneNetwork::of(vec![
            lane(90, vec![Vec2::ZERO, Vec2::new(300_000.0, 0.0)]),
            lane(91, vec![Vec2::new(150_000.0, 0.0), Vec2::new(150_000.0, 180_000.0)]),
        ]);
        let (entry, bend_a, bend_b, exit) = (
            Vec2::ZERO,
            Vec2::new(110_000.0, 0.0),
            Vec2::new(150_000.0, 40_000.0),
            Vec2::new(150_000.0, 150_000.0),
        );
        let sites = [gateway(entry), repeater(bend_a), repeater(bend_b), gateway(exit)];
        let (_, hops) = crossing.signal(entry, exit, c, &sites);
        let used: std::collections::BTreeSet<u32> =
            hops.iter().filter_map(|hop| hop.lane).collect();
        assert_eq!(used, [90, 91].into_iter().collect(), "covered wire turns freely at a junction");
        assert!(
            hops.iter().filter(|hop| hop.lane.is_some()).all(|hop| hop.to != bend_a && hop.to != bend_b),
            "junction-spanning repeaters still never become entry or exit hops",
        );
    }

    #[test]
    fn a_gap_in_the_wire_drops_the_ride_to_warp() {
        let n = signal_line(400_000.0);
        let (entry, r1, r2, exit) = (
            Vec2::ZERO,
            Vec2::new(110_000.0, 0.0),
            Vec2::new(220_000.0, 0.0),
            Vec2::new(300_000.0, 0.0),
        );
        let c = 400.0;
        let broken = [gateway(entry), repeater(r1), gateway(exit)];
        let (broken_t, broken_hops) = n.signal(entry, exit, c, &broken);
        let warp = entry.distance(exit) / (c * WARP_FACTOR);
        assert!((broken_t - warp).abs() < 1e-9);
        assert!(broken_hops.iter().all(|hop| hop.lane.is_none()));

        let repaired = [gateway(entry), repeater(r1), repeater(r2), gateway(exit)];
        let (repaired_t, repaired_hops) = n.signal(entry, exit, c, &repaired);
        assert!(repaired_t < broken_t);
        assert!(repaired_hops.iter().any(|hop| hop.lane.is_some()));
    }

    #[test]
    fn an_intact_corridor_never_drops_a_riding_hull() {
        let n = signal_line(500_000.0);
        let (entry, left, hull, right) = (
            Vec2::ZERO,
            Vec2::new(120_000.0, 0.0),
            Vec2::new(200_000.0, 0.0),
            Vec2::new(280_000.0, 0.0),
        );
        let sites = [gateway(entry), repeater(left), repeater(right)];
        let c = 400.0;
        let (delay, hops) = n.signal_to_coupled(entry, hull, c, &sites);
        let warp = entry.distance(hull) / (c * WARP_FACTOR);
        assert!(delay < warp * 0.5, "the midpoint hull remains on the covered corridor");
        assert!(hops.iter().any(|hop| hop.lane == Some(77)));
    }

    /// A coupled hull is only a terminal on wire that already reaches it. It
    /// must not contribute another buoy-sized coverage bubble of its own: once
    /// the hull crosses the selected buoy's 40k-su boundary, both orders and
    /// reports fall back to ordinary warp.
    #[test]
    fn a_riding_hull_does_not_extend_a_buoys_throw() {
        let n = signal_line(100_000.0);
        let home = Vec2::ZERO;
        let inside = Vec2::new(39_999.0, 0.0);
        let outside = Vec2::new(40_001.0, 0.0);
        let sites = [gateway(home)];
        let c = 400.0;

        let (inside_order, inside_hops) = n.signal_to_coupled(home, inside, c, &sites);
        let inside_warp = home.distance(inside) / (c * WARP_FACTOR);
        assert!(inside_order < inside_warp * 0.5, "a hull inside the buoy radius rides the wire");
        assert!(inside_hops.iter().any(|hop| hop.lane == Some(77)));

        let outside_warp = home.distance(outside) / (c * WARP_FACTOR);
        let (outside_order, outside_hops) = n.signal_to_coupled(home, outside, c, &sites);
        let outside_report = n.signal_coupled(outside, home, c, &sites);
        assert!((outside_order - outside_warp).abs() < 1e-9, "an order beyond the buoy radius uses warp");
        assert!((outside_report - outside_warp).abs() < 1e-9, "a report beyond the buoy radius uses warp");
        assert!(outside_hops.iter().all(|hop| hop.lane.is_none()));
    }

    #[test]
    fn past_the_end_of_the_wire_a_hull_gets_warp_mail() {
        let n = signal_line(500_000.0);
        let (home, r1, r2, hull) = (
            Vec2::ZERO,
            Vec2::new(120_000.0, 0.0),
            Vec2::new(280_000.0, 0.0),
            Vec2::new(401_000.0, 0.0),
        );
        let sites = [gateway(home), repeater(r1), repeater(r2)];
        let c = 400.0;
        let warp = home.distance(hull) / (c * WARP_FACTOR);
        let (outbound, hops) = n.signal_to_coupled(home, hull, c, &sites);
        let inbound = n.signal_coupled(hull, home, c, &sites);
        assert!((outbound - warp).abs() < 1e-9);
        assert!((inbound - warp).abs() < 1e-9);
        assert!(hops.iter().all(|hop| hop.lane.is_none()));
    }

    /// §comms-infra: two gateways sharing NO lane are just two points in space,
    /// so the signal stays at warp. Building them anywhere is not building wire.
    #[test]
    fn gateways_off_the_network_relay_nothing() {
        let (n, _, _, radius) = net(1, 4);
        let c = 400.0;
        let (a, b) = (Vec2::new(radius * 3.0, 0.0), Vec2::new(radius * 3.0, radius));
        let (t, hops) = n.signal(a, b, c, &[wide_gateway(a), wide_gateway(b)]);
        assert!((t - a.distance(b) / (c * WARP_FACTOR)).abs() < 1e-6, "warp, not relay");
        assert!(hops.iter().all(|h| h.lane.is_none()), "no hop claims a lane");
    }

    /// CONTAINMENT IS MEASURED TO THE CENTERLINE, not to the nearest sample.
    ///
    /// Samples sit far further apart than a ribbon is wide, so a sample-granular
    /// yardstick reports a point lying exactly ON the centerline as most of a
    /// segment away from it — and therefore outside the lane it is sitting on.
    /// That is what made a fleet riding a lane fall out of it and back in once
    /// per segment, at a factor of ten in speed each time.
    #[test]
    fn a_point_on_the_centerline_is_inside_its_own_lane() {
        let (n, ..) = net(1, 4);
        for l in &n.lanes {
            // Walk the whole route, landing BETWEEN samples as well as on them —
            // mid-segment is exactly where the old measure was worst.
            for k in 0..=200 {
                let s = l.length() * (k as f64 / 200.0);
                let p = l.at(s);
                let (_, d) = l.nearest(p).expect("a route has samples");
                assert!(
                    d <= l.half_width,
                    "{}: a point on its own centerline at s={s:.0} measured {d:.0} from it \
                     (half-width {:.0}) — the ribbon test is granular, not the geometry",
                    l.name,
                    l.half_width,
                );
            }
        }
    }

    /// SYMMETRY. A lane is not a current, so it carries information equally both
    /// ways — which is what lets ONE precomputed field serve both outbound orders
    /// and inbound reports. If lanes ever gained directional flow, this breaks and
    /// two fields would be needed.
    #[test]
    fn information_delay_is_symmetric() {
        let (n, anchors, ..) = net(4242, 4);
        let c = 400.0;
        for w in anchors.windows(2).take(12) {
            let (a, b) = (w[0].pos, w[1].pos);
            let ab = n.delay(a, b, c);
            let ba = n.delay(b, a, c);
            assert!((ab - ba).abs() < 1e-9, "delay {ab:.3} one way, {ba:.3} the other");
        }
    }

    /// Routes are CONTINUOUS ROUTES, not A↔B edges: they pass several anchors and
    /// carry one name along the whole trunk.
    ///
    /// A SPUR is deliberately exempt. It is the short filament from one home to
    /// the nearest highway, so serving exactly one system is not a degenerate
    /// route — it is the whole job, and holding it to a trunk's contract would
    /// only pressure generation into padding it with worlds it has no business
    /// visiting. What a spur owes is checked below instead.
    #[test]
    fn routes_run_past_several_systems_under_one_name() {
        let (n, anchors, ..) = net(2024, 4);
        let spacing = median_spacing(&anchors);
        assert!(n.lanes.len() >= 6, "a 4-player galaxy grows a real network, got {}", n.lanes.len());
        let mut highways = 0;
        for l in &n.lanes {
            assert!(l.control.len() >= 3, "{} is an edge, not a route", l.name);
            assert!(!l.samples.is_empty(), "{} was never baked", l.name);
            if l.kind == LaneKind::Spur {
                continue;
            }
            highways += 1;
            let near = anchors
                .iter()
                .filter(|a| {
                    l.nearest(a.pos)
                        .is_some_and(|(_, d)| d <= l.half_width + spacing * SERVED_HOP_FRAC)
                })
                .count();
            assert!(near >= 2, "{} runs past only {near} system(s)", l.name);
        }
        assert!(highways >= 5, "a network of nothing but spurs is not a network, got {highways}");
    }

    /// UNEVEN CORNERS MUST NOT OVERSHOOT. A growth walk hops anchor to anchor, so
    /// the segments meeting at a corner routinely differ by 2x or more. Under
    /// uniform spline parameterization that asymmetry bends the curve far tighter
    /// than the corner itself calls for — which is precisely how an arc whose
    /// control polygon measured a comfortable 83k baked out at 21k, well inside
    /// the fastest hull's turning circle. Centripetal knots are what hold the
    /// curve near its polygon; this is the property that says so.
    ///
    /// The bar is the radius the SAME corner would produce with even segments
    /// (`SPLINE_TURN_K * L / theta`, calibrated over 10-45 degrees). Asymmetry
    /// must not make a corner tighter than its symmetric equivalent.
    #[test]
    fn an_uneven_corner_does_not_bend_tighter_than_an_even_one() {
        const SPLINE_TURN_K: f64 = 0.49;
        let (theta, short, long) = (0.5f64, 40_000.0, 100_000.0);
        let leg = Vec2::from_polar(theta, long);
        let pts = vec![Vec2::new(-short, 0.0), Vec2::new(0.0, 0.0), leg, leg + leg];
        let even = SPLINE_TURN_K * short / theta;
        let got = route_radius(&pts);
        assert!(
            got >= even,
            "a {:.0}k-into-{:.0}k corner baked to {got:.0}, tighter than an even corner's {even:.0}",
            short / 1000.0,
            long / 1000.0,
        );
    }


    /// What a SPUR owes, since the rule above excuses it: it must actually join
    /// its home to the rest of the network. A filament that serves one system and
    /// reaches nothing is worse than no spur at all — it would read on the map as
    /// a highway connection the home does not have.
    #[test]
    fn a_spur_joins_its_home_to_a_real_route() {
        for seed in [2024u64, 1, 7] {
            let (n, _, homes, _) = net(seed, 4);
            for s in n.lanes.iter().filter(|l| l.kind == LaneKind::Spur) {
                let head = *s.control.first().unwrap();
                let tail = *s.control.last().unwrap();
                assert!(
                    homes.iter().any(|h| {
                        let d = h.distance(head);
                        (WELL_STANDOFF..=WELL_STANDOFF + s.half_width * 4.0).contains(&d)
                    }),
                    "seed {seed}: {} does not terminate on a home's standoff ring", s.name,
                );
                let joins = n.lanes.iter().filter(|o| o.id != s.id).any(|o| {
                    o.nearest(tail).is_some_and(|(_, d)| d <= o.half_width + s.half_width)
                });
                assert!(joins, "seed {seed}: {} runs from its home to nowhere", s.name);
            }
        }
    }
}

#[cfg(test)]
mod route_flight {
    use super::tests::*;
    use super::*;
    use crate::ship::{Fleet, FleetOrder, ShipKind};

    /// A ROUTED FLIGHT MUST BEAT THE STRAIGHT LINE IT REPLACED.
    ///
    /// Following a lane is only worth the detour if the fleet actually holds the
    /// lane, and it did not: arriving at a waypoint zeroed velocity, `factor`
    /// reads heading to decide a fleet is riding a lane, and a fleet at rest is
    /// riding nothing. So a routed fleet dropped to warp speed at
    /// every corner, re-aligned over the next leg, and dropped again — arriving
    /// LATER than if it had ignored the network altogether. A route that loses to
    /// a straight line is worse than no route: it burns the detour for nothing.
    #[test]
    fn following_a_route_beats_flying_straight() {
        let (hub, anchors, homes, radius) = galaxy(1, 4);
        let net = generate(1, hub, &anchors, &homes, radius);
        let wells: Vec<(Vec2, f64)> = anchors.iter().map(|a| (a.pos, HYPERLIMIT)).collect();
        let env = TransitEnv { lanes: &net, wells: &wells };

        let from = anchors[0].pos;
        let to = anchors
            .iter()
            .max_by(|a, b| a.pos.distance(from).total_cmp(&b.pos.distance(from)))
            .unwrap()
            .pos;

        let fly = |routed: bool| -> f64 {
            let mut f = Fleet::single(
                crate::EntityId(1),
                crate::PlayerId(1),
                ShipKind::Convoy,
                from,
                FleetOrder::MoveTo { dest: to },
                None,
            );
            let base = f.transit_speed();
            if routed {
                f.route = net.route(
                    from,
                    to,
                    base * WARP_FACTOR,
                    base * WARP_FACTOR * LANE_MULT,
                );
                assert!(!f.route.is_empty(), "this leg should have a lane worth taking");
            }
            let dt = 1.0 / 30.0;
            let mut t = 0.0;
            while !matches!(f.order, FleetOrder::Idle) && t < 100_000.0 {
                f.advance(t, dt, &env);
                t += dt;
            }
            t
        };

        let (straight, routed) = (fly(false), fly(true));
        assert!(
            routed < straight,
            "the routed flight took {routed:.0}s against {straight:.0}s flying straight — \
             the fleet is not holding the lane it was routed onto",
        );
    }

    /// A ROUTE ALWAYS HAS AN INSIDE, right out to its tip.
    ///
    /// The frontier taper thins a route's last stretch, which is the point — the
    /// network should fade rather than stop at a hard edge. Thinning it to ZERO
    /// is a different thing: at the terminus a fleet measures zero offset against
    /// zero half-width and is ruled outside the very lane it is sitting on, so
    /// the end of every trunk and chord silently stopped granting the benefit
    /// that made it a lane.
    #[test]
    fn a_tapered_route_still_has_an_inside_at_its_tip() {
        let (hub, anchors, homes, radius) = galaxy(1, 4);
        let net = generate(1, hub, &anchors, &homes, radius);
        for l in net.lanes.iter().filter(|l| l.tapers) {
            // A point a WHISKER off the centerline at the terminus. Testing the
            // centerline itself proves nothing: a zero-width ribbon still admits
            // an offset of exactly zero, so the degenerate case passes.
            let last = l.samples.last().unwrap();
            let n = Vec2::new(-last.tangent.y, last.tangent.x).normalized();
            let just_off = last.pos + n * 1.0;
            assert!(
                l.contains(just_off).is_some(),
                "{} rules out a point 1 su off its own terminus — the ribbon has \
                 tapered to nothing, so the end of the route is not on the route",
                l.name,
            );
        }
        // The narrowing itself still happens.
        let tapered = net.lanes.iter().find(|l| l.tapers).expect("some route tapers");
        let len = tapered.length();
        assert!(
            tapered.half_width_at(len) < tapered.half_width_at(len * 0.5),
            "the frontier should still thin out, just not to nothing",
        );
    }

    /// §course-change: REVERSING COSTS A FULL SHUTDOWN AND RESTART.
    ///
    /// A fleet cannot steer above thrusters, so turning round means dropping all
    /// the way out, coming about, and spinning back up. This is the rule that
    /// replaced the old geometric argument — keep ribbons narrower than any hull
    /// can turn inside of — which did not survive contact: past the alignment
    /// gate a hull's speed fell tenfold and its turning circle with it, so a
    /// Titan came about in ~1,000 su inside a 5,900 su ribbon without ever
    /// leaving it.
    #[test]
    fn coming_about_means_dropping_out_of_the_drive_first() {
        let net = LaneNetwork::default();
        let env = TransitEnv { lanes: &net, wells: &[] };
        let mut f = crate::ship::Fleet::single(
            crate::EntityId(1),
            crate::PlayerId(1),
            crate::ship::ShipKind::Raider,
            Vec2::ZERO,
            crate::ship::FleetOrder::MoveTo { dest: Vec2::new(400_000.0, 0.0) },
            None,
        );
        f.fuel = 1e9;
        let dt = 1.0 / 30.0;
        // Run east until the warp drive has caught.
        let mut t = 0.0;
        while t < 10.0 {
            f.advance(t, dt, &env);
            t += dt;
        }
        assert_eq!(f.regime, Regime::Warp, "under way in warp");
        assert!(!f.drive_state.can_steer(), "and unable to steer while it is");
        let east = f.vel;

        // Now send it the other way. It must NOT simply swing round.
        f.order = crate::ship::FleetOrder::MoveTo { dest: Vec2::new(-400_000.0, 0.0) };
        f.route.clear();
        f.advance(t, dt, &env);
        t += dt;
        assert!(
            matches!(f.drive_state, crate::ship::DriveState::Dropping { .. }),
            "a course reversal shuts the drive down first, got {:?}",
            f.drive_state,
        );
        assert_eq!(f.regime, Regime::Thrusters, "and it is off warp speed immediately");

        // It stays unable to steer until the shutdown finishes.
        let mut dropped_at = None;
        while t < 40.0 {
            f.advance(t, dt, &env);
            t += dt;
            if f.drive_state.can_steer() {
                dropped_at = Some(t);
                break;
            }
        }
        let dropped = dropped_at.expect("the drive does finish shutting down");
        assert!(dropped >= WARP_DROP_S, "the shutdown takes its stated time ({dropped:.1}s)");
        // And only THEN does the heading come round.
        while t < 80.0 {
            f.advance(t, dt, &env);
            t += dt;
            if f.vel.dot(east) < 0.0 {
                return; // came about, after paying for it
            }
        }
        panic!("never came about at all");
    }

    /// The mechanism the flight above depends on: CROSSING A WAYPOINT IS NOT
    /// ARRIVING. A fleet part-way through a route is still under way, so it must
    /// never be found at rest between two waypoints — velocity is what `factor`
    /// reads to grant lane speed, and it is what stops a fleet setting off in a
    /// new direction from nothing, which is how one could turn round inside a
    /// ribbon without ever leaving it.
    #[test]
    fn a_waypoint_is_crossed_not_arrived_at() {
        let (hub, anchors, homes, radius) = galaxy(1, 4);
        let net = generate(1, hub, &anchors, &homes, radius);
        let wells: Vec<(Vec2, f64)> = anchors.iter().map(|a| (a.pos, HYPERLIMIT)).collect();
        let env = TransitEnv { lanes: &net, wells: &wells };
        let from = anchors[0].pos;
        let to = anchors
            .iter()
            .max_by(|a, b| a.pos.distance(from).total_cmp(&b.pos.distance(from)))
            .unwrap()
            .pos;

        let mut f = Fleet::single(
            crate::EntityId(1),
            crate::PlayerId(1),
            ShipKind::Convoy,
            from,
            FleetOrder::MoveTo { dest: to },
            None,
        );
        let base = f.transit_speed();
        f.route =
            net.route(from, to, base * WARP_FACTOR, base * WARP_FACTOR * LANE_MULT);
        let planned = f.route.len();
        assert!(planned > 2, "need a multi-waypoint route to cross anything");


    }

    /// A destination inside the lane ribbon is a PARKING POINT in hyperspace,
    /// not an excuse to drop the drive for a tiny final warp hop. Remaining
    /// coupled keeps the ship reachable while the player chooses its next
    /// order, and that next lane-bound route must depart without cycling the
    /// drive merely because the parked point is off the exact centerline.
    #[test]
    fn an_in_ribbon_arrival_stays_coupled_for_the_next_order() {
        let control = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(150_000.0, 0.0),
            Vec2::new(300_000.0, 0.0),
        ];
        let net = LaneNetwork::of(vec![Lane {
            id: 7,
            kind: LaneKind::Trunk,
            name: "Parking Lane".into(),
            samples: bake(&control),
            control,
            half_width: 2_000.0,
            tapers: false,
        }]);
        let env = TransitEnv { lanes: &net, wells: &[] };
        let from = Vec2::new(10_000.0, 0.0);
        let destination = Vec2::new(200_000.0, 1_000.0);
        let mut fleet = Fleet::single(
            crate::EntityId(1),
            crate::PlayerId(1),
            ShipKind::Raider,
            from,
            FleetOrder::MoveTo { dest: destination },
            None,
        );
        let base = fleet.transit_speed();
        fleet.route =
            net.route(from, destination, base * WARP_FACTOR, base * WARP_FACTOR * LANE_MULT);
        assert_eq!(
            fleet.route.last().and_then(|leg| leg.lane),
            Some(7),
            "the final in-ribbon leg must not be tagged as open-space warp",
        );

        let dt = 1.0 / 30.0;
        let mut time = 0.0;
        while !matches!(fleet.order, FleetOrder::Idle) && time < 1_000.0 {
            fleet.advance(time, dt, &env);
            time += dt;
        }
        assert!(matches!(fleet.order, FleetOrder::Idle), "the fleet arrived");
        assert_eq!(fleet.pos, destination);
        assert_eq!(
            fleet.drive_state,
            crate::ship::DriveState::Cruising(Regime::Hyperspace),
            "arrival inside the ribbon keeps the hyperspace drive engaged",
        );
        assert!(
            fleet.drive_state.stirs_the_lane(),
            "the parked hull remains command-coupled"
        );

        // Waiting for the player does not quietly wind the drive down.
        for _ in 0..300 {
            fleet.advance(time, dt, &env);
            time += dt;
        }
        assert_eq!(
            fleet.drive_state,
            crate::ship::DriveState::Cruising(Regime::Hyperspace),
            "ten idle seconds leave the parked drive engaged",
        );

        // A subsequent order farther along the same ribbon uses that existing
        // coupling, including the short off-centre return to the centerline.
        let next = Vec2::new(280_000.0, 500.0);
        fleet.order = FleetOrder::MoveTo { dest: next };
        fleet.route =
            net.route(fleet.pos, next, base * WARP_FACTOR, base * WARP_FACTOR * LANE_MULT);
        assert_eq!(
            fleet.route.first().and_then(|leg| leg.lane),
            Some(7),
            "an in-ribbon origin is already on the lane",
        );
        fleet.advance(time, dt, &env);
        assert_eq!(
            fleet.drive_state,
            crate::ship::DriveState::Cruising(Regime::Hyperspace),
            "the next lane order does not cycle the drive",
        );
    }
}

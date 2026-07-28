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
pub const HYPERSPACE_FACTOR: f64 = 5.0;

/// How much faster a lane is than open hyperspace. The brief's "~10× speed and
/// fuel efficiency" — relative to off-lane hyperspace, NOT to normal space.
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
pub const GALAXY_SCALE: f64 = HYPERSPACE_FACTOR * LANE_MULT;

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

/// Ribbon width as a fraction of the median neighbouring-system distance. One
/// standardized width for v1 — no minor/major/super classes until the standard
/// model is understood.
pub const LANE_WIDTH_FRAC: f64 = 0.26;

/// The share of the tightest hull's turning circle a ribbon's half-width may
/// occupy. Below 1.0 by construction — at 1.0 a Titan could just come about
/// inside a lane, which is the one thing the width is not allowed to permit.
const RIBBON_CEILING_FRAC: f64 = 0.9;

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
/// from a world to the highway beside it. Was expressed as a multiple of the
/// gravity well back when routes had to dodge one; a well says nothing about
/// reach now that they can run straight over it, so it is stated against system
/// spacing directly. The total (ribbon + hop) is unchanged, so the density the
/// chord pass generates to is the same.
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
    /// The control points the spline passes through (already well-offset).
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

    /// Nearest centerline sample to `p`, and the distance to it.
    fn nearest(&self, p: Vec2) -> Option<(&LaneSample, f64)> {
        self.samples
            .iter()
            .map(|s| (s, s.pos.distance(p)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
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
            self.half_width * ((len - s) / taper).clamp(0.0, 1.0)
        }
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
}

impl LaneNetwork {
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

impl TransitEnv<'_> {
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
        HYPERSPACE_FACTOR * self.lanes.speed_factor(p, dir)
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
        if self.on_lane(p) { self.signal_factor_on_lane() } else { HYPERSPACE_FACTOR }
    }

    /// Signal speed inside a ribbon, as a multiple of normal-space `c`.
    fn signal_factor_on_lane(&self) -> f64 {
        HYPERSPACE_FACTOR * LANE_MULT
    }

    /// INFORMATION DELAY between two points, in seconds, at normal-space `c`.
    ///
    /// Two candidate routes, cheapest wins:
    ///   * direct through open hyperspace, and
    ///   * hop to a lane, run along it, hop off.
    ///
    /// The lane network never moves, so the middle leg is a static all-pairs
    /// problem that can be precomputed at generation; this walks it directly,
    /// which is honest and fast enough at galaxy scale. `LANE_ENTRY_CANDIDATES`
    /// bounds the search — entering a lane far away costs distance linearly
    /// while the saving is bounded, so the optimal entry is always local.
    pub fn delay(&self, a: Vec2, b: Vec2, c: f64) -> f64 {
        let direct = a.distance(b) / (c * HYPERSPACE_FACTOR);
        if self.lanes.is_empty() {
            return direct;
        }
        let lane_speed = c * self.signal_factor_on_lane();
        let open_speed = c * HYPERSPACE_FACTOR;
        let mut best = direct;
        for lane in &self.lanes {
            // Nearest point on this route to each endpoint: hop on, run, hop off.
            let (Some((sa, da)), Some((sb, db))) = (lane.nearest(a), lane.nearest(b)) else {
                continue;
            };
            let along = (sa.s - sb.s).abs() / lane_speed;
            let t = da / open_speed + along + db / open_speed;
            if t < best {
                best = t;
            }
        }
        best
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
    /// Normal-space `c`, still the anchor every other speed is expressed against.
    pub c: f64,
}

impl DelayField<'_> {
    /// Seconds for information to get from `a` to `b`.
    ///
    /// Symmetric, because a lane is not a current and carries information equally
    /// both ways — which is what lets one field serve outbound orders and inbound
    /// reports alike.
    pub fn between(&self, a: Vec2, b: Vec2) -> f64 {
        self.lanes.delay(a, b, self.c)
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
    /// `v_deep` and `v_lane` are the fleet's speeds in open hyperspace and on a
    /// lane; the comparison is made at the speeds THIS fleet can actually reach,
    /// so a slow hauler and a fast raider can legitimately choose different
    /// routes over the same pair of points.
    pub fn route(&self, from: Vec2, to: Vec2, v_deep: f64, v_lane: f64) -> Vec<Vec2> {
        let direct_t = from.distance(to) / v_deep.max(1e-9);
        let mut best: (f64, Vec<Vec2>) = (direct_t, vec![to]);

        for lane in &self.lanes {
            let (Some((a, da)), Some((b, db))) = (lane.nearest(from), lane.nearest(to)) else {
                continue;
            };
            // Riding between these two points on the centerline.
            let (s0, s1) = (a.s, b.s);
            let along = (s1 - s0).abs();
            let t = da / v_deep.max(1e-9) + along / v_lane.max(1e-9) + db / v_deep.max(1e-9);
            if t >= best.0 {
                continue;
            }
            // The centerline samples between entry and exit, in travel order —
            // this is what makes a fleet actually follow the curve.
            let (lo, hi) = if s0 <= s1 { (s0, s1) } else { (s1, s0) };
            let mut pts: Vec<Vec2> = lane
                .samples
                .iter()
                .filter(|sm| sm.s >= lo && sm.s <= hi)
                .map(|sm| sm.pos)
                .collect();
            if s0 > s1 {
                pts.reverse(); // travelling against the sampling direction
            }
            if pts.is_empty() {
                continue;
            }
            pts.push(to);
            best = (t, pts);
        }
        best.1
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
            lanes.push(finish(&mut next_id, ctrl, half_width, true, LaneKind::Trunk));
        }
    }

    // 2 — ARCS. Rings crossing the radials, so travel between two frontier
    //     systems does not have to go in to the hub and back out.
    for frac in [0.4, 0.7] {
        if let Some(ctrl) = arc(hub, radius * frac, anchors, spacing) {
            lanes.push(finish(&mut next_id, ctrl, half_width, false, LaneKind::Arc));
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
            lanes.push(finish(&mut next_id, ctrl, half_width, true, LaneKind::Chord));
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
        let start = *h + along * (half_width * 2.0);
        let ctrl = vec![start, start + (join - start) * 0.5, join];
        lanes.push(finish(&mut next_id, ctrl, half_width, false, LaneKind::Spur));
    }

    LaneNetwork { lanes }
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
    // The margin absorbs the well offsets `finish` applies afterwards, which
    // perturb the ring again on the way to the final geometry.
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
    half_width: f64,
    tapers: bool,
    kind: LaneKind,
) -> Lane {
    let id = *next_id;
    *next_id += 1;
    // NO POST-PROCESSING. A route is exactly what it was grown to be.
    //
    // This used to be a relaxation between two constraints that could not both be
    // met: dodge every gravity well, and stay inside the fastest hull's turning
    // circle. Lanes may now run straight over a system, so the dodge is gone —
    // and with it the smoothing, the tether and the retreat that existed only to
    // arbitrate the two. What is left is what the module always claimed: a route
    // is navigable BY CONSTRUCTION, because a growth walk caps its turn and
    // floors its leg length, an arc measures itself against the bar before it
    // settles, and a spur is three collinear points. Nothing downstream can bend
    // a route into something no hull can fly, because nothing downstream bends it
    // at all.
    Lane {
        id,
        kind,
        name: route_name(id),
        samples: bake(&ctrl),
        control: ctrl,
        half_width,
        tapers,
    }
}

/// The fastest hull's lane speed — what every route must be navigable at. Kept
/// here rather than imported so `lane` stays free of the ship table; the value
/// is asserted against `ship::max_speed` by test.
fn fastest_lane_speed() -> f64 {
    115.0 * HYPERSPACE_FACTOR * LANE_MULT
}

/// The slowest hull's lane speed, and so the TIGHTEST circle any hull can turn
/// inside a ribbon. A route's half-width has to stay under it — see `ribbon_half_width`.
fn slowest_lane_speed() -> f64 {
    23.0 * HYPERSPACE_FACTOR * LANE_MULT
}

/// How wide a ribbon may be, given the systems it threads.
///
/// Width is a fraction of system spacing, but it is CAPPED, and the cap is the
/// reason in-lane reversal is impossible rather than merely discouraged. To come
/// about inside a corridor of width `2h` a hull needs a turning circle of `r <= h`;
/// keep `h` below the tightest circle any hull can turn — the Titan's — and the
/// manoeuvre has nowhere to happen. "Exit hyperspace, swing round, re-enter" then
/// costs what it costs because geometry says so, not because a rule forbids it.
///
/// The cap matters because spacing is not a fixed quantity: it moves with player
/// count and with the luck of a seed, so a width fraction that clears the circle
/// in a 4-player galaxy can breach it in a 6-player one. Tuning the fraction
/// against a measured ceiling would leave the invariant one unlucky galaxy from
/// failing; deriving it here means no galaxy can produce a ribbon a hull can turn
/// inside of.
fn ribbon_half_width(spacing: f64) -> f64 {
    let from_spacing = spacing * LANE_WIDTH_FRAC * 0.5;
    // Margin so the ribbon stays clearly under the circle rather than equal to it.
    let ceiling = TURN_K * slowest_lane_speed() * RIBBON_CEILING_FRAC;
    from_spacing.min(ceiling)
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
    let net = LaneNetwork { lanes: lanes.to_vec() };
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

    /// A galaxy shaped like the real generator's: area-uniform systems in an
    /// annulus, homes on a ring, hub at the origin.
    pub(super) fn galaxy(seed: u64, players: usize) -> (Vec2, Vec<LaneAnchor>, Vec<Vec2>, f64) {
        let radius = 4000.0 * (players as f64).sqrt() * HYPERSPACE_FACTOR * 10.0;
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
                let scout_lane_speed = 115.0 * HYPERSPACE_FACTOR * LANE_MULT;
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

    /// NO IN-LANE U-TURNS, checked against the SLOWEST hull — the one with the
    /// tightest circle, and therefore the only one that could ever come about
    /// inside a ribbon. This is what makes "reversal costs you an exit and an
    /// arc" a geometric fact rather than a rule someone has to enforce.
    #[test]
    fn no_hull_can_come_about_inside_a_ribbon() {
        // ACROSS PLAYER COUNTS AND SEEDS. Width is capped against this circle
        // rather than tuned to clear it, and the cap is what makes the rule
        // structural — but median spacing moves with the roster and with the luck
        // of a seed, so a single 4-player galaxy is not evidence the cap holds.
        // Measured, the spacing that drives width varies by better than 8%
        // between rosters, which is enough to swallow a hand-tuned margin.
        let titan_lane_speed = 23.0 * HYPERSPACE_FACTOR * LANE_MULT;
        let tightest = TURN_K * titan_lane_speed;
        for players in [2usize, 3, 4, 5, 6, 8] {
            for seed in 0u64..8 {
                let (n, ..) = net(seed, players);
                for l in &n.lanes {
                    assert!(
                        tightest > l.half_width,
                        "{players}p seed {seed}: {} is {:.0} half-wide, and a Titan turns in {tightest:.0} — it could come about without ever leaving the lane",
                        l.name,
                        l.half_width,
                    );
                }
            }
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
        let n = LaneNetwork {
            lanes: vec![
                straight(0, vec![Vec2::new(-9000.0, 0.0), Vec2::ZERO, Vec2::new(9000.0, 0.0)]),
                straight(1, vec![Vec2::new(0.0, -9000.0), Vec2::ZERO, Vec2::new(0.0, 9000.0)]),
                straight(2, vec![Vec2::new(-9000.0, 60.0), Vec2::new(0.0, 60.0), Vec2::new(9000.0, 60.0)]),
            ],
        };
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
        let n = LaneNetwork { lanes: vec![l] };
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
        let net = LaneNetwork { lanes: vec![l] };
        let env = TransitEnv { lanes: &net, wells: &[] };
        let on = Vec2::ZERO;
        let off = Vec2::new(0.0, 40_000.0);
        let east = Vec2::new(1.0, 0.0);

        assert_eq!(env.factor(on, east, false), 1.0, "drive off: normal space, wherever you are");
        assert_eq!(env.factor(off, east, true), HYPERSPACE_FACTOR, "drive on, off-lane: open hyperspace");
        assert_eq!(
            env.factor(on, east, true),
            HYPERSPACE_FACTOR * LANE_MULT,
            "drive on, aligned on a lane: the highway",
        );
        assert_eq!(
            env.factor(on, Vec2::new(0.0, 1.0), true),
            HYPERSPACE_FACTOR,
            "crossing the lane earns open-hyperspace speed only — the alignment gate",
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
            HYPERSPACE_FACTOR,
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
        let speed = 40.0 * HYPERSPACE_FACTOR * LANE_MULT;
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
            let speed = base * HYPERSPACE_FACTOR * LANE_MULT;
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
            ("open hyperspace", c * HYPERSPACE_FACTOR, fastest * HYPERSPACE_FACTOR),
            (
                "lane",
                c * HYPERSPACE_FACTOR * LANE_MULT,
                fastest * HYPERSPACE_FACTOR * LANE_MULT,
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
        let net = LaneNetwork { lanes: vec![lane] };
        let (a, b) = (Vec2::ZERO, Vec2::new(span, 0.0));

        let on_lane = net.delay(a, b, c);
        let off_lane = LaneNetwork::default().delay(a, b, c);
        assert!(
            (off_lane / on_lane - LANE_MULT).abs() < 0.05 * LANE_MULT,
            "on the trunk {on_lane:.1}s vs open hyperspace {off_lane:.1}s — expected ~{LANE_MULT}×",
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
            cfg.galaxy_radius / (cfg.c * HYPERSPACE_FACTOR * LANE_MULT)
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
        let off_lane = cfg.galaxy_radius / (cfg.c * HYPERSPACE_FACTOR);
        assert!(
            (off_lane / base - LANE_MULT).abs() < 0.01,
            "off-lane should be exactly {LANE_MULT}× slower than a trunk, got {off_lane:.0}s vs {base:.0}s",
        );
        // ...and in normal space, slower again by the hyperspace factor.
        let sublight = cfg.galaxy_radius / cfg.c;
        assert!(
            (sublight / off_lane - HYPERSPACE_FACTOR).abs() < 0.01,
            "normal space should be {HYPERSPACE_FACTOR}× slower again, got {sublight:.0}s",
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
        let net = LaneNetwork {
            lanes: vec![Lane {
                id: 0,
                kind: LaneKind::Trunk,
                name: "Bow".into(),
                samples: bake(&ctrl),
                control: ctrl,
                half_width: 2_000.0,
                tapers: false,
            }],
        };
        let (from, to) = (Vec2::new(0.0, 0.0), Vec2::new(200_000.0, 0.0));
        let (v_deep, v_lane) = (200.0, 2_000.0);

        let route = net.route(from, to, v_deep, v_lane);
        assert!(route.len() > 2, "a lane route is a path, not a single hop");
        assert_eq!(*route.last().unwrap(), to, "and it always ends where it was sent");

        // It genuinely BOWS: the straight line is y=0 throughout, so any real
        // deviation proves the fleet is following the curve rather than the chord.
        let peak = route.iter().map(|p| p.y.abs()).fold(0.0, f64::max);
        assert!(peak > 20_000.0, "the route hugs the lane's bow (peak y = {peak:.0})");

        // And it is chosen because it is FASTER despite being longer.
        let path_len: f64 = std::iter::once(from)
            .chain(route.iter().copied())
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

    /// When no lane helps, the route is a straight run — the common case near
    /// home, and the fallback that keeps every pre-lane behaviour intact.
    #[test]
    fn a_route_with_no_useful_lane_is_a_straight_run() {
        let net = LaneNetwork::default();
        let (from, to) = (Vec2::new(0.0, 0.0), Vec2::new(50_000.0, 0.0));
        assert_eq!(net.route(from, to, 200.0, 2_000.0), vec![to]);
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
                    homes.iter().any(|h| h.distance(head) <= s.half_width * 4.0),
                    "seed {seed}: {} starts at no home", s.name,
                );
                let joins = n.lanes.iter().filter(|o| o.id != s.id).any(|o| {
                    o.nearest(tail).is_some_and(|(_, d)| d <= o.half_width + s.half_width)
                });
                assert!(joins, "seed {seed}: {} runs from its home to nowhere", s.name);
            }
        }
    }
}

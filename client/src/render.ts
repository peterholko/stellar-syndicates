// Pixi.js renderer. Draws the player's DELAYED, FOGGED view (§6) — the heart of
// the game made visible (Pillar 2: never hide the lag). Each ship is a ghost at
// the position its arriving light shows; EVERY ghost — own or enemy — carries an
// age label and fades with staleness. Own fleets have three light-honest map
// states: a delayed full sprite in the bubble, an arrived-light arrow in dark
// realspace, and a stationary entry-bookmark arrow while coupled hyperspace lies
// beyond the wire. Rivals retain the ordinary fogged view.

import { Application, Assets, Container, Graphics, Sprite, Text, TextStyle, Texture } from "pixi.js";
import { label } from "./icons";
import type { BodyView, EmplacementView, GalaxyInfo, GhostView, LaneView, PathPointView, ShipKind, SystemInfo, Vec2 } from "./protocol";
import { countClassLabel, fleetExactCount } from "./protocol";
import { ghostInTunnel, liveSimTime, type ViewState } from "./state";
import { STAR_TYPES, starAnchor, starIconUrl, starTypeFor, starVisualRatio } from "./stars";
import { buildVisualSystem, SystemViewScene, type SystemBodyDetail } from "./systemview";

// --- SEMANTIC-ZOOM VIEW MODE (galaxy ⇄ system) --------------------------------
// The renderer hosts TWO scenes with INDEPENDENT coordinate systems: the galaxy
// map (unchanged: `scale`/`cx`/`cy` camera + all gameplay layers) and a schematic
// System View (its own fixed fit camera, in systemview.ts). Only one is active at
// a time; a crossfade + camera push connects them. This is a LEVEL-OF-DETAIL
// change, NOT a second scale of gameplay — see the hard-boundary note in
// systemview.ts. Ships/convoys/raiders/fog/combat/movement ALL stay on the galaxy
// map; the System View is presentation only.
export type MapViewMode =
  | { type: "galaxy" }
  | { type: "system"; systemId: string }
  | { type: "battle"; battleId: string };

// Crossfade + camera-push transition between the two scenes. Autopilot preserves
// the established 480ms click/Esc/battle motion; a system handoff can instead
// let the wheel drive the same progress, easing, camera, and alpha curves.
const TRANS_MS = 480;
const GALAXY_OVERSHOOT = 1.6; // system handoff: separations keep opening as the galaxy fades
const SCHEMATIC_GROW_FROM = 0.35; // system handoff: planets arrive outward from their star
const easeInOut = (t: number): number => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2);
const clamp01 = (x: number): number => Math.max(0, Math.min(1, x));
interface Transition {
  dir: "in" | "out";
  target: "system" | "battle";
  driver: "autopilot" | "scrub";
  id: string;
  /// System transitions magnify about this star. Battle transitions omit it.
  focus?: Vec2;
  start: number;
  camFrom: { cx: number; cy: number; scale: number };
  camTo: { cx: number; cy: number; scale: number };
}

const COL_HUB = 0x7fd4ff;
const COL_SYSTEM = 0x4a5d7a;

// §explore: the MAP reads only the PUBLIC richness band (the free spectral
// read, same for everyone) — 3 star sizes + 3 glow radii. Exact deposits are
// per-corp knowledge and live in the PANEL, never on the shared map (the old
// deposit-value sizing + dominant-resource tint were geology leaks).
const BAND_SIZE: Record<string, number> = { poor: 22, fair: 32, rich: 44 };
const BAND_GLOW: Record<string, number> = { poor: 5, fair: 10, rich: 16 };
export const COL_OWN = 0x4fc3ff; // exported: the battle theater reuses the team palette
export const COL_OTHER = 0xff7a6b; // exported: the battle theater reuses the team palette
// §syndicates: a SYNDICATE ally — a friendly GREEN, distinct from own cyan and
// rival red (and from the teal sensor bubbles). Applied per the viewer's
// light-delayed membership knowledge (the `ally` view flag).
const COL_ALLY = 0x74e08c;
// §pirates: the neutral PIRATE faction — a menacing AMBER-ORANGE, distinct from
// own cyan / ally green / rival salmon. "Nobody's friend."
const COL_PIRATE = 0xe08a2c;
// §TCA: the TERRAN CHARTER AUTHORITY — a cool institutional STEEL-BLUE, read as
// "neutral officialdom": not yours, not a rival, not a pirate. Its freighters are
// the only hulls that wear it.
const COL_TCA = 0x7fa6c8;
// §node: EXOTIC NODES — a VIOLET exotic accent, distinct from every faction hue.
// The glyph marks a node; ownership stays on the system ring/tint.
const COL_NODE = 0xb98cff;
const COL_ANCHOR_OWN = 0x9be7ff;
const COL_ANCHOR_OTHER = 0xcf9b6b;
const COL_COMMAND = 0xc56bff; // outbound order comet (violet)
const COL_ROUTE = 0x8fe3a0; // own flight plan (green, legible over blue lane bands)
const COL_INTENT_ROUTE = 0x70ad7d; // pending intent, dimmer than an observed flight plan
const COL_ROUTE_PREVIEW = 0xc3f7cc; // prospective route, lighter than the committed plan
const COL_COMMS_DARK = 0xe2ad62; // stale own-contact warning, distinct from route/lane hues
const COL_IMPULSE_BOUNDARY = 0xf0a64a; // gravity-well edge, warm against lane blue
const COL_REPORT = 0xffd24a; // known convoy cargo label (gold = intel)
const COL_THREAT = 0xff4d4d; // detected raider (alert red)
const COL_ESTIMATE = 0xffae5c; // crude intercept estimate (soft amber, fuzzy)
// Ships render in their NATURAL art — no per-syndicate body tint (a future
// ownership indicator is TBD). This neutral is only the primitive fallback hull
// shown before the sprite art loads; it must NOT imply ownership.
const COL_SHIP_NEUTRAL = 0xc9d6e8;

const MAX_EXTRAPOLATE_S = 0.4;
const FADE_AGE_S = 45; // staleness at which an enemy ghost is most faded
const REENTRY_FX_MS = 500; // Tunable: dark arrow catches the newly arrived live picture.
const DARK_ARROW_SCALE = 2; // Tunable: frozen fleet chevron, relative to its original glyph.
const DARK_DELAY_ICON_PX = 36; // Fixed screen px; the 128px source stays crisp on high-DPI displays.

// --- Zoom limits, as multiples of the fit-to-galaxy scale (so they scale with
// galaxy size). MIN ≈ fit (whole galaxy visible, a touch looser); MAX resolves
// one system's local well geometry before the schematic handoff. ---
const ZOOM_MIN_FACTOR = 0.9;
const ZOOM_MAX_FACTOR = 96;
const MACHINE_ZOOM_END = 24;
// Battle markers keep the old semantic doorway (0.62 × the former 24× max).
// Extending the system approach must not push battle entry four times deeper.
const BATTLE_ZOOM_FACTOR = MACHINE_ZOOM_END * 0.62;

interface GhostSprite {
  container: Container;
  cone: Graphics;
  body: Graphics; // primitive triangle — fallback until the ship sprite loads
  sprite: Sprite; // the ship art (rotated to heading, tinted by ownership)
  delayIcon: Sprite; // fixed-screen communication-delay cue beside a dark arrow
  delayTooltip: Container; // hover explanation for the canvas-only delay cue
  label: Text;
  ring: Graphics; // selection ring
  pip: Graphics; // ownership tag (cyan = yours, red = rival) — the friend/foe cue
  badge: Graphics; // fleet count pill (exact Σ, or the fog size bucket)
  badgeText: Text; // the number / bucket label drawn on the badge
  seen: boolean;
  /// §hyperspace: the WORLD position actually drawn, which chases the
  /// authoritative one rather than jumping to it. See `drawGhost`.
  shown?: { x: number; y: number };
  /// Previous server-authoritative presentation mode, for edge transitions.
  inComms?: boolean;
  /// Previous client-derived tunnel predicate. Unlike `inComms`, this also
  /// includes the served drive fact and therefore owns bookmark/reappear edges.
  inTunnel?: boolean;
  darkMorphMs?: number;
  /// The dark→live catch-up. Both endpoints are served player knowledge.
  reentry?: { from: Vec2; to: Vec2; startedMs: number };
}

interface ReacquireFx {
  root: Container;
  pulse: Graphics;
  oldPos: Vec2;
  newPos: Vec2;
  startedMs: number;
}

// §perf: pooled per-system draw objects (drawSystems). One `g` carries ALL the
// system's geometry (glow, ownership rings, blockade/enclave/node dashes,
// selection ring, dot fallback) exactly as the old per-frame Graphics did; the
// label + three optional tag texts are pooled and toggled by visibility. Reused
// across frames via clear()+redraw, so the drawn result is identical with zero
// per-frame allocation.
interface SystemGfx {
  g: Graphics;
  label: Text;
  blockade: Text; // "⛔ BLOCKADE" tag
  enclave: Text; // "☠ ENCLAVE T‹n›" tag
  node: Text; // "◈" dormant glyph OR "◈ TITLE" awakened tag
}

// Ship sprites are top-down with the nose at -y; the heading convention here points
// +x at angle 0, so rotate the sprite by +90° to align its nose with the heading.
const SHIP_ART_FACING = Math.PI / 2;

// On-map ship sprite sizes (screen px at the fit zoom) — big enough that the
// detailed art reads, with the convoy clearly LARGER than the nimble raider.
// Tunable. They scale modestly with zoom (clamped) so they stay sensible.
const SHIP_PX_CONVOY = 56;
const SHIP_PX_RAIDER = 40;
const SHIP_PX_CORVETTE = 48; // between raider and convoy — the size hierarchy
const SHIP_PX_COLONY = 64; // the biggest thing flying
const SHIP_PX_SCOUT = 30; // the smallest hull on the map
const SHIP_ZOOM_MIN = 0.9; // shrink floor when zoomed out
// §hyperspace: how hard a drawn fleet chases its authoritative position. Higher
// converges faster and shows more of each correction; lower is smoother but lags.
// §roads: how much of the sim's swept lane width the drawn band occupies.
// Purely visual — the gameplay tolerance is unchanged.
const LANE_DRAW_FRAC = 0.45;
// Mirrors sim lane::HYPERLIMIT — every star system is a drive-forbidden well.
const HYPERLIMIT_SU = 900;
// §emplacements: kept in step with `emplace::MIN_SPACING`. (Standing sensors
// draw their coverage from the wire's `sensor_range` — no mirrored radius.)
const EMPLACEMENT_MIN_SPACING = 12_000;
// Completed open-space structures use dedicated 256px art. At normal zoom they
// sit between the scout and freighter markers; deep zoom grows them enough to
// inspect without letting stationary infrastructure rival a star or the hub.
const EMPLACEMENT_PX = 34;
const EMPLACEMENT_MAX_PX = 96;
const SMOOTH_RATE = 9.0; // e-folds per second
// RIVALS ONLY: corrections bigger than this snap rather than ease. A rival
// re-appearing from fog really is new information at a new place — easing it
// would paint positions you never observed. YOUR OWN fleets never snap: a
// fleet riding a lane toward you outruns its own report (hyperspace ×50 vs a
// buoy-less warp signal ×5), so its bow-wave of light arrives essentially
// WITH the ship and the correction is the WHOLE leg — no fixed threshold can
// contain it, which is how the first version of this reintroduced the very
// teleport it claimed to fix. Easing closes any distance in ~half a second,
// reading as the ship streaking home. (The in-fiction cure is a relay pair on
// the lane: relayed reports at ×50 outrun any hull, and the blackout never
// happens at all.)
const SMOOTH_SNAP_SU = 4_000;
const SMOOTH_SNAP_S = 0.75; // ...or this many seconds of its own travel, whichever is larger
const SHIP_ZOOM_MAX = 1.6; // indicator growth cap (normal-zoom phase)
// §zoom-continuum has distinct machine and body bands. Machines retain the
// established 12→24 ramp and then freeze. Bodies stay at map-indicator size
// throughout that neighborhood and almost all of the 24→96 approach; only the
// final quarter blooms them to their unchanged System View-matched endpoints.
// Between r=24 and r=72 no object grows, so world-space separation does the work.
const SHIP_NATIVE_ZOOM_START = 12;
const BODY_ZOOM_START = 0.75 * ZOOM_MAX_FACTOR;

// §size-hierarchy: per-class size targets at the ends of their own bands.
const SHIP_MAX_PX = 120; // a ship at max zoom (was: the art's native 256 — too big next to bodies)
// The hub has NO fixed max target: its deep-zoom ceiling is the landmark
// texture's NATIVE width (1254px), so max zoom renders it at sprite scale
// exactly 1.0 — pixel-crisp by construction, never upscaled (a fixed target
// above the asset's resolution is what made it blurry). See hubRenderedPx.
// Click-target cap for grown BODIES: a max-zoom star/hub is hundreds of px, and
// its hit circle stops here.
//
// §dock: this cap existed because ships PARKED ON a star competed with it for
// clicks. Berthed hulls are no longer drawn on the star chart at all, so that
// pressure is gone and the cap could be loosened considerably — a max-zoom star
// could reasonably be clickable across its whole disc now. Kept deliberately:
// a fleet at a BLOCKADED berth still draws (decluttering must not conceal an
// attack), and a fleet merely passing near a star is still hit-tested first.
// The cap costs nothing and keeps those two cases clickable.
const BODY_HIT_CAP_PX = 90;

// §battle-aftermath tunables. The marker is SCREEN-SPACE UI (like pips/badges):
// it never grows in the deep-zoom band. TTL hides ancient markers (the report
// stays in the retained list / results log until the server rotates it out).
// Battle marker on-screen sizes (screen px — SET HERE, not by the texture
// resolution; the sprite is scaled to this size regardless of the source PNG's
// dimensions). Doubled from the original 22/26 so the icons read clearly on the
// galaxy map. Tunable.
const BATTLE_MARKER_PX = 44; // aftermath / capture icon size on screen
const BATTLE_MARKER_TTL_S = 1800; // hide markers learned > 30 min ago (tunable)
const BATTLE_MARKER_HIT_PX = 24; // click radius (scaled with the bigger markers)
const BATTLE_ONGOING_PX = 52; // the in-progress icon size (pulse scales it a bit)
// §aftermath-fade: a concluded-battle marker fades with time since the viewer's
// report ARRIVED — full (with the fresh pulse) at first, a smooth decay over
// AFTERMATH_FADE_SECS down to AFTERMATH_FLOOR_ALPHA, then held at that floor
// (still selectable) until BATTLE_MARKER_TTL_S removes it. Old battles literally
// fade into the dark. Both tunable.
const AFTERMATH_FADE_SECS = 240;
const AFTERMATH_FLOOR_ALPHA = 0.15;

// FLEET FORMATION sprites (§fleet-art): a fleet marker draws a formation image —
// lead ship + escorts — picked by the flagship's FAMILY and a size TIER derived
// from what the VIEWER knows (exact count when own/in-coverage, else the fog
// bucket). 1 ship → the single-ship sprite exactly as before; colony fleets have
// no formation art and always fall back to single sprite + count badge.
type FleetTier = "wing" | "squadron" | "armada";
type FleetFamily = "freighter" | "raider" | "corvette" | "scout";
// Per-tier designer multipliers on the formation canvas (relative feel knobs —
// e.g. make armadas read a touch grander). 1.0 = lead-ship parity (see below).
const TIER_SCALE: Record<FleetTier, number> = { wing: 1.0, squadron: 1.0, armada: 1.0 };
// Measured per-sprite calibration = (single sprite's subject height fraction) /
// (formation's LEAD-ship height fraction), so the LEAD ship renders at exactly
// the single sprite's on-screen size — no size pop when a fleet crosses a tier
// boundary (e.g. 3 → 4 ships). Derived from the shipped art; remeasure if the
// art changes.
const FLEET_LEAD_CALIB: Record<FleetFamily, Record<FleetTier, number>> = {
  freighter: { wing: 0.95, squadron: 1.08, armada: 0.99 },
  raider: { wing: 0.86, squadron: 0.92, armada: 1.02 },
  corvette: { wing: 0.95, squadron: 0.81, armada: 1.06 },
  scout: { wing: 0.81, squadron: 1.07, armada: 0.96 },
};

// §fleet-lod: at far zoom-out the detailed hull/formation art muddies down to a
// few dozen pixels, so below this zoom ratio r (= scale / fitScale) the
// freighter / raider / corvette fleets swap to bold LOW-DETAIL ICONS that read
// cleanly when tiny (the count badge still carries fleet size). Scout and colony
// have no icon yet and keep their detailed art at every zoom. Tunable — raise to
// keep icons through more of the zoom range, lower for an earlier hand-off to the
// detailed art + formations.
const LOD_ICON_ZOOM_MAX = 2.5;
// The icon families (a subset of FleetFamily — scout is excluded, no icon yet).
type LodFamily = "freighter" | "raider" | "corvette";
// Per-family calibration so the icon's SUBJECT renders at ~the detailed single
// sprite's on-screen size across the swap (no size pop). 1.0 = canvas-width
// parity with the single sprite; nudge if a family visibly jumps at the hand-off.
const LOD_ICON_CALIB: Record<LodFamily, number> = {
  freighter: 1.0,
  raider: 1.0,
  corvette: 1.0,
};

/// Project a served sighting onto the flight plan the same sighting conveyed.
/// `next` is the waypoint whose incoming segment contains the projection, so
/// consumers can continue forward without replaying already-flown legs.
function nearestOnPath(
  pos: { x: number; y: number },
  path: PathPointView[],
): { next: number; x: number; y: number } {
  let nearest = { next: 0, x: pos.x, y: pos.y };
  if (path.length < 2) return nearest;

  let bestD = Infinity;
  let nearestT = 0;
  for (let i = 1; i < path.length; i++) {
    const a = path[i - 1].pos;
    const b = path[i].pos;
    const abx = b.x - a.x;
    const aby = b.y - a.y;
    const len2 = abx * abx + aby * aby;
    const t = len2 > 1e-9
      ? Math.max(0, Math.min(1, ((pos.x - a.x) * abx + (pos.y - a.y) * aby) / len2))
      : 0;
    const x = a.x + abx * t;
    const y = a.y + aby * t;
    const d = Math.hypot(pos.x - x, pos.y - y);
    if (d < bestD) {
      bestD = d;
      nearestT = t;
      nearest = { next: i, x, y };
    }
  }

  // `LaneNetwork::route` deliberately omits its origin. On a newly-started
  // course the sighting can therefore still be approaching path[0], outside
  // the waypoint-only polyline above. A projection clamped to the beginning of
  // its first segment identifies that case: retain the sighting as the anchor
  // and let consumers draw/creep through the otherwise-missing first leg.
  if (nearest.next === 1 && nearestT <= 1e-9) {
    return { next: 0, x: pos.x, y: pos.y };
  }
  return nearest;
}

type ArcInterval = { lo: number; hi: number };
type LaneArc = { lane: LaneView; s: number[]; length: number };

function laneArcs(lanes: LaneView[]): Map<number, LaneArc> {
  const out = new Map<number, LaneArc>();
  for (const lane of lanes) {
    const s = [0];
    for (let i = 1; i < lane.points.length; i++) {
      s.push(s[i - 1] + Math.hypot(
        lane.points[i].x - lane.points[i - 1].x,
        lane.points[i].y - lane.points[i - 1].y,
      ));
    }
    out.set(lane.id, { lane, s, length: s[s.length - 1] ?? 0 });
  }
  return out;
}

function projectLane(p: Vec2, arc: LaneArc): { s: number; d: number } | null {
  let best: { s: number; d: number } | null = null;
  for (let i = 1; i < arc.lane.points.length; i++) {
    const a = arc.lane.points[i - 1];
    const b = arc.lane.points[i];
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const len2 = dx * dx + dy * dy;
    const t = len2 > 1e-9 ? clamp01(((p.x - a.x) * dx + (p.y - a.y) * dy) / len2) : 0;
    const q = { x: a.x + dx * t, y: a.y + dy * t };
    const d = Math.hypot(p.x - q.x, p.y - q.y);
    if (!best || d < best.d) best = { s: arc.s[i - 1] + (arc.s[i] - arc.s[i - 1]) * t, d };
  }
  return best;
}

function segmentClosest(
  a0: Vec2,
  a1: Vec2,
  b0: Vec2,
  b1: Vec2,
): { ta: number; tb: number; distance: number } {
  const ad = { x: a1.x - a0.x, y: a1.y - a0.y };
  const bd = { x: b1.x - b0.x, y: b1.y - b0.y };
  const den = ad.x * bd.y - ad.y * bd.x;
  if (Math.abs(den) >= 1e-9) {
    const delta = { x: b0.x - a0.x, y: b0.y - a0.y };
    const ta = (delta.x * bd.y - delta.y * bd.x) / den;
    const tb = (delta.x * ad.y - delta.y * ad.x) / den;
    if (ta >= 0 && ta <= 1 && tb >= 0 && tb <= 1) return { ta, tb, distance: 0 };
  }
  const project = (p: Vec2, s0: Vec2, s1: Vec2): { t: number; distance: number } => {
    const dx = s1.x - s0.x;
    const dy = s1.y - s0.y;
    const len2 = dx * dx + dy * dy;
    const t = len2 > 1e-9 ? clamp01(((p.x - s0.x) * dx + (p.y - s0.y) * dy) / len2) : 0;
    return { t, distance: Math.hypot(p.x - (s0.x + dx * t), p.y - (s0.y + dy * t)) };
  };
  const a0b = project(a0, b0, b1);
  const a1b = project(a1, b0, b1);
  const b0a = project(b0, a0, a1);
  const b1a = project(b1, a0, a1);
  return [
    { ta: 0, tb: a0b.t, distance: a0b.distance },
    { ta: 1, tb: a1b.t, distance: a1b.distance },
    { ta: b0a.t, tb: 0, distance: b0a.distance },
    { ta: b1a.t, tb: 1, distance: b1a.distance },
  ].reduce((best, candidate) => candidate.distance < best.distance ? candidate : best);
}

function laneHalfWidthAt(arc: LaneArc, s: number): number {
  if (!arc.lane.tapers || arc.length <= 1e-9) return arc.lane.half_width;
  const taperLength = arc.length * 0.15;
  const remaining = arc.length - s;
  if (remaining >= taperLength) return arc.lane.half_width;
  const t = clamp01(remaining / taperLength);
  return arc.lane.half_width * (0.35 * (1 - t) + t);
}

function mergeIntervals(intervals: ArcInterval[]): ArcInterval[] {
  const sorted = intervals.slice().sort((a, b) => a.lo - b.lo);
  const out: ArcInterval[] = [];
  for (const interval of sorted) {
    const last = out[out.length - 1];
    if (last && interval.lo <= last.hi + 1e-6) last.hi = Math.max(last.hi, interval.hi);
    else out.push({ ...interval });
  }
  return out;
}

/// Mirror the public relay geometry for the map legend: green graph-distance
/// wire over the otherwise untouched road. The relay's 2D circle is the binary
/// live/dark boundary; there is no intermediate lane-earshot marker tier.
function commLaneZones(
  lanes: LaneView[],
  emplacements: EmplacementView[],
): Map<number, ArcInterval[]> {
  const arcs = laneArcs(lanes);
  const junctions: { on: { lane: number; s: number }[] }[] = [];
  for (let ai = 0; ai < lanes.length; ai++) {
    const a = arcs.get(lanes[ai].id)!;
    for (let bi = ai + 1; bi < lanes.length; bi++) {
      const b = arcs.get(lanes[bi].id)!;
      const candidates: { ia: number; ib: number; sa: number; sb: number; distance: number }[] = [];
      for (let i = 1; i < a.lane.points.length; i++) {
        for (let j = 1; j < b.lane.points.length; j++) {
          const hit = segmentClosest(
            a.lane.points[i - 1], a.lane.points[i],
            b.lane.points[j - 1], b.lane.points[j],
          );
          const sa = a.s[i - 1] + (a.s[i] - a.s[i - 1]) * hit.ta;
          const sb = b.s[j - 1] + (b.s[j] - b.s[j - 1]) * hit.tb;
          if (hit.distance <= laneHalfWidthAt(a, sa) + laneHalfWidthAt(b, sb)) {
            candidates.push({ ia: i - 1, ib: j - 1, sa, sb, distance: hit.distance });
          }
        }
      }
      // Match the sim's physical junction rule: overlapping ribbons join even
      // when their centerlines narrowly miss. Adjacent overlapping segment
      // pairs are one junction, represented by their closest handoff.
      while (candidates.length > 0) {
        const cluster = [candidates.pop()!];
        for (;;) {
          const before = candidates.length;
          for (let i = candidates.length - 1; i >= 0; i--) {
            const candidate = candidates[i];
            if (cluster.some((other) =>
              Math.abs(other.ia - candidate.ia) <= 1 && Math.abs(other.ib - candidate.ib) <= 1)) {
              cluster.push(candidate);
              candidates.splice(i, 1);
            }
          }
          if (candidates.length === before) break;
        }
        const best = cluster.reduce((x, y) => y.distance < x.distance ? y : x);
        junctions.push({
          on: [{ lane: a.lane.id, s: best.sa }, { lane: b.lane.id, s: best.sb }],
        });
      }
    }
  }

  const zones = new Map<number, ArcInterval[]>();
  for (const lane of lanes) zones.set(lane.id, []);
  const sites = emplacements.filter((e) =>
    e.own !== false && e.relay_throw > 0
    && (e.kind === "hyperspace_buoy" || e.kind === "hyperspace_repeater"));
  for (const site of sites) {
    const starts: { lane: number; s: number }[] = [];
    for (const arc of arcs.values()) {
      const at = projectLane(site.pos, arc);
      if (!at || at.d > arc.lane.half_width) continue;
      starts.push({ lane: arc.lane.id, s: at.s });
    }
    const distance = junctions.map((junction) => Math.min(...junction.on.flatMap((on) =>
      starts.filter((start) => start.lane === on.lane).map((start) => Math.abs(start.s - on.s)))));
    const done = new Array(junctions.length).fill(false);
    for (;;) {
      let u = -1;
      for (let i = 0; i < distance.length; i++) {
        if (!done[i] && Number.isFinite(distance[i]) && (u < 0 || distance[i] < distance[u])) u = i;
      }
      if (u < 0) break;
      done[u] = true;
      for (let v = 0; v < junctions.length; v++) {
        if (done[v]) continue;
        let edge = Infinity;
        for (const a of junctions[u].on) for (const b of junctions[v].on) {
          if (a.lane === b.lane) edge = Math.min(edge, Math.abs(a.s - b.s));
        }
        if (distance[u] + edge < distance[v]) distance[v] = distance[u] + edge;
      }
    }
    const anchors = new Map<number, { s: number; base: number }[]>();
    for (const start of starts) {
      const list = anchors.get(start.lane) ?? [];
      list.push({ s: start.s, base: 0 });
      anchors.set(start.lane, list);
    }
    junctions.forEach((junction, i) => {
      if (!Number.isFinite(distance[i])) return;
      for (const on of junction.on) {
        const list = anchors.get(on.lane) ?? [];
        list.push({ s: on.s, base: distance[i] });
        anchors.set(on.lane, list);
      }
    });
    for (const [laneId, laneAnchors] of anchors) {
      const arc = arcs.get(laneId)!;
      for (const anchor of laneAnchors) {
        const reach = site.relay_throw - anchor.base;
        if (reach < 0) continue;
        zones.get(laneId)!.push({
          lo: Math.max(0, anchor.s - reach),
          hi: Math.min(arc.length, anchor.s + reach),
        });
      }
    }
  }
  for (const [laneId, zone] of zones) zones.set(laneId, mergeIntervals(zone));
  return zones;
}

function laneSpan(arc: LaneArc, interval: ArcInterval): Vec2[] {
  const pointAt = (s: number): Vec2 => {
    let i = arc.s.findIndex((x) => x >= s);
    if (i <= 0) return arc.lane.points[0];
    if (i < 0) return arc.lane.points[arc.lane.points.length - 1];
    const span = arc.s[i] - arc.s[i - 1];
    const t = span > 1e-9 ? (s - arc.s[i - 1]) / span : 0;
    return {
      x: arc.lane.points[i - 1].x + (arc.lane.points[i].x - arc.lane.points[i - 1].x) * t,
      y: arc.lane.points[i - 1].y + (arc.lane.points[i].y - arc.lane.points[i - 1].y) * t,
    };
  };
  const out = [pointAt(interval.lo)];
  for (let i = 1; i < arc.s.length - 1; i++) {
    if (arc.s[i] > interval.lo && arc.s[i] < interval.hi) out.push(arc.lane.points[i]);
  }
  out.push(pointAt(interval.hi));
  return out;
}

// The WORMHOLE HUB map sprite (§hub-art): the game's most important location
// reads as a LANDMARK — clearly the largest body on the map at normal zoom
// (stars top out at 46px), holds through the approach, then blooms to its native
// landmark size over r=72→96 (§zoom-continuum).
const HUB_PX = 72;
/// Fraction of the hub sprite's canvas its visible subject fills (measured).
const HUB_ART_FILL = 0.93;

export class Renderer {
  private app = new Application();
  // A persistent starfield behind BOTH scenes (never faded), so the backdrop is
  // continuous across the galaxy⇄system LOD change.
  private starfield = new Container();
  // The galaxy scene root — ALL existing gameplay layers live under it, so the
  // whole galaxy can be faded/pushed as one during the transition. The galaxy
  // camera (scale/cx/cy) still drives everything inside it exactly as before.
  private galaxyRoot = new Container();
  private bg = new Container(); // galaxy rings + hub (was: also the starfield)
  /// §hyperspace: the lane ribbons. Static geometry, so this is rebuilt only
  /// when the camera moves — not per frame.
  private lanesGfx = new Graphics();
  private laneBoundaryWorld = new Map<number, {
    source: Vec2[];
    halfWidth: number;
    tapers: boolean;
    left: Vec2[];
    right: Vec2[];
  }>();
  /// §emplacements: coverage/fallback glyphs, dedicated structure art, then
  /// selection chrome. Keeping these as separate children puts the selection
  /// ring above the opaque sprite while coverage remains beneath it.
  private emplacementLayer = new Container();
  private emplaceGfx = new Graphics();
  private emplacementSpriteLayer = new Container();
  private emplaceSelectionGfx = new Graphics();
  private emplacementSprites = new Map<string, Sprite>();
  /// World-space cursor, for the siting preview. Null when the pointer is off-map.
  cursorWorld: { x: number; y: number } | null = null;
  private routesGfx = new Graphics();
  private systemsLayer = new Container();
  private anchorsLayer = new Container();
  private orderLayer = new Container();
  // §perf: persistent Graphics/Text reused across frames (clear()+redraw instead
  // of removeChildren()+new every frame). Eliminates the per-frame allocation
  // churn — the drawn output is identical, only the object churn is gone.
  private orderGfx = new Graphics(); // convoy order lines (pooled)
  private anchorGfx = new Graphics(); // anchor circles (pooled)
  private ccGfx = new Graphics(); // command-center pulse (pooled)
  private homeText: Text | null = null; // the viewer's "HOME" seat label (pooled)
  private bgGfx: Graphics | null = null; // hub glow rings (pooled)
  private hubText: Text | null = null; // the "HUB" label (pooled)
  // §perf: pooled per-system draw objects, keyed by system id. Rebuilt only when
  // geometry is dirty (camera / new View / selection) or an animated system is
  // present — never allocated per frame.
  private systemGfx = new Map<string, SystemGfx>();
  private interceptGfx = new Graphics(); // soft intercept-estimate zones
  // §battle-aftermath: concluded-battle markers (owner-only UI chrome) — under
  // the ghosts (a marker never hides a ship), over bodies/estimates.
  private aftermathLayer = new Container();
  private aftermathGfx = new Graphics();
  private aftermathSprites = new Map<number, Sprite>();
  private battleSprites = new Map<string, Sprite>(); // pooled ongoing-battle icons, keyed by engagement id
  private battleHits: { id: string; sx: number; sy: number }[] = []; // §one-battle-one-icon click targets
  private aftermathHits: { id: number; sx: number; sy: number }[] = [];
  // §aftermath-select: the concluded-battle marker (aftermath OR capture report id)
  // that currently carries the standard selection ring. Set by main.ts on click,
  // cleared when any other object is selected. Ring draws at full even at the fade
  // floor, so an all-but-faded marker is still visibly selectable.
  selectedBattleMarkerId: number | null = null;
  private captureHits: { id: number; sx: number; sy: number }[] = []; // §Part 2 capture markers
  private ghostsLayer = new Container();
  private reacquireLayer = new Container();
  private reacquireFx: ReacquireFx[] = [];
  private signalsLayer = new Container();
  private signalsGfx = new Graphics();
  private interceptLabels = new Map<string, Text>();
  private commandSignalLabels = new Map<number, Text>();
  private ghosts = new Map<string, GhostSprite>();
  private servedGhostFrames = new Map<string, { pos: Vec2; simTime: number; pinned: boolean }>();

  // Celestial body sprites (planet = star system, station = hub), pooled in a
  // persistent layer UNDER the ownership/value/label cues so those still read.
  private bodyLayer = new Container();
  private systemBodies = new Map<string, Sprite>();
  private hubSprite: Sprite | null = null;
  // Star-type map icons, keyed by slug — a system draws the icon for its
  // deterministically-assigned type (stars.ts). Loaded lazily in loadArt.
  private starTex = new Map<string, Texture>();
  private texStation: Texture | null = null;
  private texHub: Texture | null = null; // the wormhole aperture + station landmark
  private texHyperspaceBuoy: Texture | null = null;
  private texHyperspaceRepeater: Texture | null = null;
  private texDeepSpaceSensor: Texture | null = null;
  // Ship sprites (convoy = freighter, raider = attack ship), top-down (nose = -y).
  private texConvoy: Texture | null = null;
  private texRaider: Texture | null = null;
  private texCorvette: Texture | null = null;
  private texColony: Texture | null = null;
  private texScout: Texture | null = null;
  // §ladder: the capital ladder (sliced from the capital-ships sheet).
  private texDestroyer: Texture | null = null;
  private texCruiser: Texture | null = null;
  private texBattleship: Texture | null = null;
  private texDreadnought: Texture | null = null;
  private texTitan: Texture | null = null;
  // §fleet-lod: bold low-detail fleet icons shown at far zoom-out (freighter =
  // convoy, raider, corvette). Null → the detailed art is used at every zoom.
  private texIconFreighter: Texture | null = null;
  private texIconRaider: Texture | null = null;
  private texIconCorvette: Texture | null = null;
  private texCommsDelay: Texture | null = null;
  // Fleet formation sprites, keyed `${family}_${tier}` (12 = 4 families × 3
  // tiers). A missing entry falls back to the single-ship sprite + badge.
  private texFleet = new Map<string, Texture>();
  // §battle-aftermath: the two battle icons (in-progress / aftermath). Null →
  // the drawn fallback markers keep working (the established art idiom).
  private texBattleOngoing: Texture | null = null;
  private texBattleAftermath: Texture | null = null;

  // The schematic System View scene (its own camera). Presentation only.
  private systemScene = new SystemViewScene();
  private mode: MapViewMode = { type: "galaxy" };
  private transition: Transition | null = null;
  /// Semantic progress: 0 = galaxy endpoint, 1 = system/battle endpoint. Timed
  /// autopilot and wheel scrub are merely two drivers of this same value.
  private transitionRaw = 0;
  private transitionTargetRaw = 0;
  private transitionLastMs = 0;
  private scrubGalaxyCam: { cx: number; cy: number; scale: number } | null = null;
  private scrubEndpoint: { type: "system"; systemId: string } | { type: "galaxy" } | null = null;
  private systemFocus: Vec2 | null = null;

  private galaxy: GalaxyInfo | null = null;
  private scale = 1;
  /// Seconds since the previous rendered frame — see the smoothing in `drawGhost`.
  private frameDt = 1 / 60;
  private lastFrameMs = 0;
  private cx = 0;
  private cy = 0;
  /// True once the user has zoomed/panned — so a window resize PRESERVES their
  /// view (re-clamping scale) instead of snapping back to fit-to-galaxy.
  private userView = false;
  /// The world-anchored background (galaxy rings + hub) is drawn only when the
  /// transform changes, not every frame; this flags it for redraw.
  private viewDirty = false;
  private laneCommsSignature = "";

  // §perf: dirty-gating. `stateVersion` is bumped by main.ts each time a new View
  // is applied; the static galaxy geometry (systems/anchors) is rebuilt only when
  // the camera moved (viewDirty), the state changed, or the selection changed —
  // and, for the systems layer, when an animated system (rival breath / blockade
  // pulse) is on screen. Idle frames with none of these do no geometry work.
  stateVersion = 0;
  private lastStateVersion = -1;
  private lastSelShip: string | null = null;
  private lastSelSystem: string | null = null;
  private lastSelEmplacement: string | null = null;
  private lastSelMarker: number | null = null;
  /// True after a rebuild in which some system drew a per-frame pulse (rival
  /// breath / blockade), so the systems layer keeps redrawing to animate it.
  private systemsAnimating = false;

  async init(mount: HTMLElement): Promise<void> {
    await this.app.init({
      background: "#05070d",
      resizeTo: window,
      antialias: true,
      autoDensity: true,
      resolution: window.devicePixelRatio || 1,
    });
    mount.appendChild(this.app.canvas);
    this.emplacementLayer.addChild(
      this.emplaceGfx,
      this.emplacementSpriteLayer,
      this.emplaceSelectionGfx,
    );
    // Galaxy scene: all existing gameplay layers under ONE root so it can be
    // faded/pushed as a unit during the semantic-zoom transition. Draw order and
    // the per-layer camera math are unchanged — only the parent is now galaxyRoot.
    this.galaxyRoot.addChild(
      this.bg,
      this.lanesGfx, // §hyperspace: lanes are TERRAIN — beneath everything
      this.emplacementLayer, // ...and what you built on them sits just above
      this.bodyLayer, // celestial body sprites, under the data cues that decorate them
      this.systemsLayer,
      this.anchorsLayer,
      this.routesGfx, // convoy broadcast routes, under ghosts
      this.orderLayer,
      this.interceptGfx, // soft intercept estimate, under the ghosts it guides
      this.aftermathLayer, // §battle-aftermath markers, under the ghosts
      this.reacquireLayer, // old contact + arrival pulse, beneath the fresh marker
      this.ghostsLayer,
      this.signalsLayer,
    );
    this.aftermathLayer.addChild(this.aftermathGfx);
    // §perf: pooled persistent graphics for the order + anchor + command-center
    // layers (drawn via clear()+redraw, never re-allocated per frame). Child order
    // preserves the old draw order: anchor circles, then HOME label, then the
    // command-center pulse on top.
    this.orderLayer.addChild(this.orderGfx);
    this.homeText = new Text({ text: "HOME", style: new TextStyle({ fill: COL_ANCHOR_OWN, fontFamily: "ui-monospace, monospace", fontSize: 10, fontWeight: "700", letterSpacing: 2 }) });
    this.homeText.anchor.set(0.5, 1);
    this.homeText.visible = false;
    this.anchorsLayer.addChild(this.anchorGfx, this.homeText, this.ccGfx);
    // Stage: persistent starfield (bottom) · galaxy scene · system scene (top,
    // hidden until entered). The HUD/breadcrumb/panels are DOM (the "hudRoot"),
    // and persist across both scenes.
    this.app.stage.addChild(this.starfield, this.galaxyRoot, this.systemScene.root);
    this.signalsLayer.addChild(this.signalsGfx);
    this.drawStarfield();
    // Load the art set (transparent PNGs from /art, bundled by Vite in dev + dist).
    // Non-blocking: the map draws (primitives) immediately and swaps to sprites the
    // moment the textures resolve — so a slow load never blanks the map.
    void this.loadArt();
    window.addEventListener("resize", () => {
      this.recompute();
      this.systemScene.layout(this.viewW, this.viewH); // the System View has its own fit camera
    });
    this.systemScene.layout(this.viewW, this.viewH);
  }

  /// Load the celestial + ship sprite textures. Each resolves independently; the
  /// draw paths guard on `tex* !== null`, so missing/slow art degrades gracefully.
  private async loadArt(): Promise<void> {
    const load = async (url: string): Promise<Texture | null> => {
      try {
        return await Assets.load(url);
      } catch {
        return null; // leave null — the primitive fallback keeps the map working
      }
    };
    // A star SYSTEM draws its assigned star-type icon (12 types). The hub is the
    // trade station. habitable_planet / sun are intentionally NOT loaded — reserved
    // for a future habitable-world / market-body concept, not generic systems.
    const [hub, station, hyperspaceBuoy, hyperspaceRepeater, deepSpaceSensor, convoy, raider, corvette, colony, scout] = await Promise.all([
      load("/art/wormhole_hub.png"),
      load("/art/celestial_sprites/mining_station.png"),
      load("/art/celestial_sprites/hyperspace_buoy.png"),
      load("/art/celestial_sprites/hyperspace_communication_buoy.png"),
      load("/art/celestial_sprites/deep_space_sensor.png"),
      load("/art/ship_sprites/cargo_freighter.png"),
      load("/art/ship_sprites/raider_attack_ship.png"),
      load("/art/ship_sprites/corvette_escort_ship.png"),
      load("/art/ship_sprites/colony_ship.png"),
      load("/art/ship_sprites/scout_utility_ship.png"),
    ]);
    // §ladder: the capital ladder, same 256px top-down/nose-up idiom.
    const [destroyer, cruiser, battleship, dreadnought, titan] = await Promise.all([
      load("/art/ship_sprites/destroyer_line_ship.png"),
      load("/art/ship_sprites/cruiser_line_ship.png"),
      load("/art/ship_sprites/battleship_line_ship.png"),
      load("/art/ship_sprites/dreadnought_line_ship.png"),
      load("/art/ship_sprites/titan_flagship.png"),
    ]);
    this.texDestroyer = destroyer;
    this.texCruiser = cruiser;
    this.texBattleship = battleship;
    this.texDreadnought = dreadnought;
    this.texTitan = titan;
    // The landmark is ONE 1254px texture drawn from a ~72px marker all the way
    // up to native 1:1 — enable mipmap generation so the minified marker keeps
    // trilinear filtering (no shimmer/aliasing at normal zoom); linear mag
    // filtering (Pixi's default) covers the crisp native view at max zoom.
    if (hub) hub.source.autoGenerateMipmaps = true;
    this.texHub = hub;
    this.texStation = station;
    // These 256px structure cutouts spend most of their life at ~34px. Mipmaps
    // keep engraved edges and antenna spars stable while the camera moves.
    for (const t of [hyperspaceBuoy, hyperspaceRepeater, deepSpaceSensor]) {
      if (t) t.source.autoGenerateMipmaps = true;
    }
    this.texHyperspaceBuoy = hyperspaceBuoy;
    this.texHyperspaceRepeater = hyperspaceRepeater;
    this.texDeepSpaceSensor = deepSpaceSensor;
    this.texConvoy = convoy;
    this.texRaider = raider;
    this.texCorvette = corvette;
    this.texColony = colony;
    this.texScout = scout;
    // §battle-aftermath: the battle-state icons (background-removed, downscaled
    // to 256 — they render at ~22-26px screen-space and never grow). The drawn
    // fallback markers still cover a failed/missing load.
    const [battleOngoing, battleAftermath] = await Promise.all([
      load("/art/battle_in_progress.png"),
      load("/art/battle_aftermath.png"),
    ]);
    this.texBattleOngoing = battleOngoing;
    this.texBattleAftermath = battleAftermath;
    // §fleet-lod: the far-zoom low-detail icons. They render at ~a few dozen px
    // from a large source, so enable mipmaps for shimmer-free minification (same
    // as the hub landmark). A missing file just leaves the detailed art in place.
    const [iconFreighter, iconRaider, iconCorvette] = await Promise.all([
      load("/art/ship_sprites/icon_freighter.png"),
      load("/art/ship_sprites/icon_raider.png"),
      load("/art/ship_sprites/icon_corvette.png"),
    ]);
    for (const t of [iconFreighter, iconRaider, iconCorvette]) {
      if (t) t.source.autoGenerateMipmaps = true;
    }
    this.texIconFreighter = iconFreighter;
    this.texIconRaider = iconRaider;
    this.texIconCorvette = iconCorvette;
    // This UI cue is positioned in ghost-container screen pixels, so its source
    // resolution serves high-DPI displays without making it grow with map zoom.
    const commsDelay = await load("/art/ui_icons/png/128/concept-communication-delay.png");
    if (commsDelay) commsDelay.source.autoGenerateMipmaps = true;
    this.texCommsDelay = commsDelay;
    // Fleet formation sprites (family × tier); each independent, missing ones
    // fall back to the single-ship sprite so a bad file never breaks fleets.
    const families: FleetFamily[] = ["freighter", "raider", "corvette", "scout"];
    const tiers: FleetTier[] = ["wing", "squadron", "armada"];
    await Promise.all(
      families.flatMap((f) =>
        tiers.map(async (t) => {
          const tex = await load(`/art/ship_sprites/fleet_${f}_${t}.png`);
          if (tex) this.texFleet.set(`${f}_${t}`, tex);
        }),
      ),
    );
    // The star-type icons (each independent; a missing one falls back to the dot).
    await Promise.all(
      STAR_TYPES.map(async (t) => {
        const tex = await load(starIconUrl(t));
        if (tex) this.starTex.set(t.slug, tex);
      }),
    );
    // §perf: the star icons drive the (dirty-gated) systems layer — force one
    // rebuild so they replace the dot fallbacks even if the player is idle.
    this.viewDirty = true;
  }

  get canvas(): HTMLCanvasElement {
    return this.app.canvas;
  }

  private get viewW(): number {
    return this.app.renderer.width / this.app.renderer.resolution;
  }
  private get viewH(): number {
    return this.app.renderer.height / this.app.renderer.resolution;
  }

  worldToScreen(p: Vec2): { x: number; y: number } {
    return { x: this.cx + p.x * this.scale, y: this.cy + p.y * this.scale };
  }
  screenToWorld(sx: number, sy: number): Vec2 {
    return { x: (sx - this.cx) / this.scale, y: (sy - this.cy) / this.scale };
  }

  /// The fit-to-galaxy scale (whole galaxy comfortably visible) — the default and
  /// reset view, and the basis for the zoom clamp.
  private fitScale(): number {
    if (!this.galaxy) return 1;
    return (Math.min(this.viewW, this.viewH) * 0.46) / this.galaxy.radius;
  }
  private clampScale(s: number): number {
    const fit = this.fitScale();
    return Math.max(fit * ZOOM_MIN_FACTOR, Math.min(fit * ZOOM_MAX_FACTOR, s));
  }

  /// Multiplicative zoom keeping the world point under (`screenX`,`screenY`) fixed
  /// (zoom toward the cursor). All draws follow via the shared transform.
  zoomAt(screenX: number, screenY: number, factor: number): void {
    if (!this.galaxy || this.isSystemScrubbing()) return;
    const before = this.screenToWorld(screenX, screenY);
    this.scale = this.clampScale(this.scale * factor);
    this.cx = screenX - before.x * this.scale;
    this.cy = screenY - before.y * this.scale;
    this.userView = true;
    this.viewDirty = true;
  }
  /// Zoom toward the viewport centre (for the +/− buttons).
  zoomByFactor(factor: number): void {
    this.zoomAt(this.viewW / 2, this.viewH / 2, factor);
  }
  /// Pan by a screen-pixel delta (drag).
  panBy(dx: number, dy: number): void {
    if (this.isSystemScrubbing()) return;
    this.cx += dx;
    this.cy += dy;
    this.userView = true;
    this.viewDirty = true;
  }
  /// Reset to the fit-to-galaxy view (and let subsequent resizes re-fit again).
  resetView(): void {
    if (this.isSystemScrubbing()) return;
    this.userView = false;
    this.recompute();
  }

  // --- Semantic-zoom (galaxy ⇄ system) — presentation only ------------------
  get viewMode(): MapViewMode {
    return this.mode;
  }
  /// True when the galaxy camera is at its deepest zoom — the cue for "zoom in
  /// again to enter the System View" (see main.ts's wheel handler).
  atMaxZoom(): boolean {
    return this.scale >= this.fitScale() * ZOOM_MAX_FACTOR - 1e-3;
  }
  /// Ongoing battles get their own semantic-zoom band: deep enough that the
  /// player is unmistakably aiming at the marker, but before a system's final
  /// inspection threshold. The next inward gesture over the marker enters the
  /// theater; stars retain their existing max-zoom doorway unchanged.
  atBattleZoomThreshold(): boolean {
    return this.scale >= this.fitScale() * BATTLE_ZOOM_FACTOR - 1e-3;
  }
  /// Camera to restore when leaving the System View (the player's pre-enter view).
  private savedGalaxyCam: { cx: number; cy: number; scale: number } | null = null;

  private systemEndpointCamera(sys: SystemInfo): { cx: number; cy: number; scale: number } {
    const scale = this.fitScale() * ZOOM_MAX_FACTOR;
    return { cx: this.viewW / 2 - sys.pos.x * scale, cy: this.viewH / 2 - sys.pos.y * scale, scale };
  }

  private prepareSystemScene(sys: SystemInfo, bodies: BodyView[]): void {
    this.systemFocus = { ...sys.pos };
    const st = starTypeFor(sys.id);
    this.systemScene.setSystem(buildVisualSystem(sys, bodies), this.starTex.get(st.slug) ?? null);
    this.systemScene.layout(this.viewW, this.viewH);
    this.systemScene.root.visible = true;
    this.systemScene.root.alpha = 0;
  }

  private startTransition(tr: Transition, raw: number, targetRaw: number): void {
    this.transition = tr;
    this.transitionRaw = raw;
    this.transitionTargetRaw = targetRaw;
    this.transitionLastMs = performance.now();
    this.galaxyRoot.visible = true;
  }

  /// ENTER the schematic System View for a system: build its visual schematic
  /// from the WIRE ROSTER (§bodies — public geography; deposits arrive
  /// survey-gated, owner data server-fogged), save the current galaxy camera,
  /// and start the crossfade + camera-push toward the star.
  enterSystemView(sys: SystemInfo, bodies: BodyView[]): void {
    if (this.mode.type === "system" && this.mode.systemId === sys.id) return;
    this.prepareSystemScene(sys, bodies);
    this.savedGalaxyCam = { cx: this.cx, cy: this.cy, scale: this.scale };
    this.scrubGalaxyCam = null;
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    // Push the galaxy camera to center the star at max zoom, so the map visibly
    // dives toward it as the schematic fades in (an LOD change that FEELS
    // connected — not a literal zoom through astronomical space).
    const camTo = this.systemEndpointCamera(sys);
    this.mode = { type: "system", systemId: sys.id };
    this.userView = true;
    this.startTransition(
      { dir: "in", target: "system", driver: "autopilot", id: sys.id, focus: sys.pos, start: performance.now(), camFrom, camTo },
      0,
      1,
    );
  }

  /// EXIT back to the galaxy, restoring the pre-enter camera as the schematic
  /// crossfades out and the galaxy pulls back.
  exitSystemView(): void {
    if (this.mode.type !== "system") return;
    const restore = this.savedGalaxyCam ?? { cx: this.viewW / 2, cy: this.viewH / 2, scale: this.fitScale() };
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    this.systemScene.clearSelection();
    this.scrubGalaxyCam = null;
    const focus = this.systemFocus ?? this.screenToWorld(this.viewW / 2, this.viewH / 2);
    this.mode = { type: "galaxy" };
    this.startTransition(
      { dir: "out", target: "system", driver: "autopilot", id: this.systemScene.currentId() ?? "", focus, start: performance.now(), camFrom, camTo: restore },
      1,
      0,
    );
  }

  /// Begin the wheel-driven handoff without changing mode/UI. The first inward
  /// tick prepares the same scene and cameras as timed entry; subsequent ticks
  /// only move the target progress. Pan/ordinary zoom stay locked until an end.
  beginSystemScrubIn(sys: SystemInfo, bodies: BodyView[]): boolean {
    if (this.mode.type !== "galaxy" || this.transition !== null || !this.atMaxZoom()) return false;
    this.prepareSystemScene(sys, bodies);
    const galaxyCam = { cx: this.cx, cy: this.cy, scale: this.scale };
    this.savedGalaxyCam = galaxyCam;
    this.scrubGalaxyCam = galaxyCam;
    this.userView = true;
    this.startTransition(
      { dir: "in", target: "system", driver: "scrub", id: sys.id, focus: sys.pos, start: performance.now(), camFrom: galaxyCam, camTo: this.systemEndpointCamera(sys) },
      0,
      0,
    );
    return true;
  }

  /// Start a wheel retreat from System View. A system reached by wheel reverses
  /// to its exact max-zoom approach camera; a timed/click entry lands on a fresh
  /// max-zoom camera centered on the star rather than restoring a shallower view.
  beginSystemScrubOut(sys: SystemInfo): boolean {
    if (this.mode.type !== "system" || this.mode.systemId !== sys.id || this.transition !== null) return false;
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    const camTo = this.scrubGalaxyCam ?? this.systemEndpointCamera(sys);
    this.startTransition(
      { dir: "out", target: "system", driver: "scrub", id: sys.id, focus: sys.pos, start: performance.now(), camFrom, camTo },
      1,
      1,
    );
    return true;
  }

  isSystemScrubbing(): boolean {
    return this.transition?.target === "system" && this.transition.driver === "scrub";
  }

  adjustSystemScrub(delta: number): void {
    if (!this.isSystemScrubbing()) return;
    this.transitionTargetRaw = clamp01(this.transitionTargetRaw + delta);
  }

  /// Esc in the handoff band completes the retreat to the galaxy endpoint.
  cancelSystemScrub(): void {
    if (this.isSystemScrubbing()) this.transitionTargetRaw = 0;
  }

  consumeSystemScrubEndpoint(): { type: "system"; systemId: string } | { type: "galaxy" } | null {
    const endpoint = this.scrubEndpoint;
    this.scrubEndpoint = null;
    return endpoint;
  }

  /// ENTER the light-delayed battle theater. The theater itself is a DOM/Pixi
  /// overlay owned by main.ts; this renderer supplies the same camera-push and
  /// galaxy crossfade language as System View, centered on the observed marker.
  enterBattleView(battleId: string, pos: Vec2): void {
    if (this.mode.type === "battle" && this.mode.battleId === battleId) return;
    this.savedGalaxyCam = { cx: this.cx, cy: this.cy, scale: this.scale };
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    // Battle keeps the pre-continuum camera depth; only systems use the 96× approach.
    const toScale = this.fitScale() * MACHINE_ZOOM_END;
    const camTo = { cx: this.viewW / 2 - pos.x * toScale, cy: this.viewH / 2 - pos.y * toScale, scale: toScale };
    this.mode = { type: "battle", battleId };
    this.userView = true;
    this.startTransition(
      { dir: "in", target: "battle", driver: "autopilot", id: battleId, start: performance.now(), camFrom, camTo },
      0,
      1,
    );
  }

  /// EXIT a semantic battle view, restoring the exact galaxy camera from which
  /// it was entered. Replay overlays opened by clicking never change viewMode.
  exitBattleView(): void {
    if (this.mode.type !== "battle") return;
    const restore = this.savedGalaxyCam ?? { cx: this.viewW / 2, cy: this.viewH / 2, scale: this.fitScale() };
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    const battleId = this.mode.battleId;
    this.mode = { type: "galaxy" };
    this.startTransition(
      { dir: "out", target: "battle", driver: "autopilot", id: battleId, start: performance.now(), camFrom, camTo: restore },
      1,
      0,
    );
  }

  /// Hit-test a planet/moon in the System View (opens a details panel — the ONLY
  /// planet interaction; no per-planet gameplay, no deeper camera level).
  systemPick(sx: number, sy: number): SystemBodyDetail | null {
    return this.systemScene.pickBody(sx, sy);
  }

  /// §management-home: forward the OWNER-ONLY development tiers to the scene's
  /// decorative structure markers (null for rival/unclaimed systems — a rival's
  /// System View stays pure scenery; the caller sources tiers from the same
  /// light-gated view fields the management panel reads).
  setSystemDynamic(bodies: BodyView[], builds: { key: string; body_id: number }[], fed: boolean): void {
    this.systemScene.setDynamic(bodies, builds, fed);
  }

  /// The contextual-build helper: which developments would ANCHOR at this visual
  /// body in the CURRENT System View (presentation sugar for the panel).
  /// §explore: takes the viewer's known deposits (owner-only surface in practice).
  // (§bodies: the anchor bridges are gone — panels read the wire roster.)

  /// §body-management: resolve a visual body id to its detail (chip → panel).
  systemBodyDetail(bodyId: string): SystemBodyDetail | null {
    return this.systemScene.detailFor(bodyId);
  }

  /// §body-management: pulse a body sprite (the chip-click affordance).
  pulseSystemBody(bodyId: string): void {
    this.systemScene.pulseBody(bodyId);
  }

  setGalaxy(galaxy: GalaxyInfo): void {
    this.galaxy = galaxy;
    // Drop pooled body sprites from any previous galaxy (fresh systems / ids) —
    // the per-system bodies AND the hub, so a galaxy change leaves no stale sprite.
    for (const sp of this.systemBodies.values()) sp.destroy();
    this.systemBodies.clear();
    this.hubSprite?.destroy();
    this.hubSprite = null;
    // §perf: drop the pooled per-system draw objects — a new galaxy has fresh
    // system ids; they're recreated lazily on the next (forced) rebuild.
    this.systemsLayer.removeChildren();
    for (const e of this.systemGfx.values()) {
      e.g.destroy();
      e.label.destroy();
      e.blockade.destroy();
      e.enclave.destroy();
      e.node.destroy();
    }
    this.systemGfx.clear();
    this.systemsAnimating = false;
    this.laneCommsSignature = "";
    this.viewDirty = true; // force a full systems rebuild against the new galaxy
    this.recompute();
  }

  private recompute(): void {
    if (!this.galaxy) return;
    if (this.userView) {
      // Preserve the user's pan/zoom across a resize; just re-clamp the scale to
      // the new viewport's limits.
      this.scale = this.clampScale(this.scale);
    } else {
      this.scale = this.fitScale();
      this.cx = this.viewW / 2;
      this.cy = this.viewH / 2;
    }
    this.drawBackground();
    // Systems are redrawn per-frame in update() (ownership/stockpile are dynamic).
  }

  private drawStarfield(): void {
    const stars = new Graphics();
    let s = 0x12345;
    const rand = () => {
      s = (s * 1103515245 + 12345) & 0x7fffffff;
      return s / 0x7fffffff;
    };
    for (let i = 0; i < 360; i++) {
      stars.circle(rand() * 2400, rand() * 1500, rand() * 1.3 + 0.2).fill({ color: 0xb8c6dd, alpha: rand() * 0.4 + 0.12 });
    }
    // Persistent backdrop shared by both scenes (no longer inside `bg`).
    this.starfield.addChild(stars);
  }

  /// §hyperspace: draw the LANE RIBBONS.
  ///
  /// Terrain, not objects — they sit beneath every gameplay layer and never
  /// move, so this rebuilds only when the camera does. Each route is drawn as a
  /// road band plus a brighter centerline; inspection zoom also reveals the
  /// wider mechanical tolerance where lane physics applies. The corridor never
  /// fades with zoom: at deep zoom its faint band legitimately fills the view.
  private drawLanes(state: ViewState): void {
    const BOUNDARY_ALPHA = 0.07;
    const BOUNDARY_FADE_START = 8;
    const BOUNDARY_FADE_END = 14;
    const TAPER_FLOOR_FRAC = 0.35;
    const g = this.lanesGfx;
    g.clear();
    const lanes = state.galaxy?.lanes ?? [];
    if (lanes.length === 0) return;
    const arcs = laneArcs(lanes);
    const commZones = commLaneZones(lanes, state.emplacements);
    const zoomR = this.scale / this.fitScale();
    const boundaryT = clamp01((zoomR - BOUNDARY_FADE_START) / (BOUNDARY_FADE_END - BOUNDARY_FADE_START));
    const boundaryAlpha = BOUNDARY_ALPHA * boundaryT * boundaryT * (3 - 2 * boundaryT);
    for (const lane of lanes) {
      if (lane.points.length < 2) continue;
      const pts = lane.points.map((p) => this.worldToScreen(p));
      const trace = () => {
        g.moveTo(pts[0].x, pts[0].y);
        for (let i = 1; i < pts.length; i++) g.lineTo(pts[i].x, pts[i].y);
      };
      // THE SWEPT WIDTH IS NOT A FREE DIAL. It is capped in the sim against the
      // tightest turning circle any hull has, because a corridor wider than that
      // circle is one a fleet could come about inside — and reversal is supposed
      // to cost an exit and an arc. So the corridor cannot simply be drawn
      // broader to give the map more presence.
      //
      // The decorative bleed was removed because it is not lane geometry; it
      // can return later as deliberate visual polish.
      // §roads: the DRAWN band is the road, not the tolerance. Ships ride the
      // centerline now, so a band at the full swept width read as a highway
      // nobody uses the shoulders of — and with the width cap gone it grew
      // heavier still. The sim's half_width keeps its full size for what it
      // still governs (boarding, buoy siting, junction detection); the siting
      // preview shows THAT band's legality itself, so nothing is hidden by
      // drawing the road slimmer than the rule.
      const widthPx = Math.max(1.5, lane.half_width * 2 * this.scale * LANE_DRAW_FRAC);
      if (boundaryAlpha >= 0.005) {
        let boundary = this.laneBoundaryWorld.get(lane.id);
        if (
          !boundary
          || boundary.source !== lane.points
          || boundary.halfWidth !== lane.half_width
          || boundary.tapers !== lane.tapers
        ) {
          const cumulative = [0];
          const segmentNormals: Vec2[] = [];
          for (let i = 1; i < lane.points.length; i++) {
            const dx = lane.points[i].x - lane.points[i - 1].x;
            const dy = lane.points[i].y - lane.points[i - 1].y;
            const len = Math.hypot(dx, dy);
            cumulative.push(cumulative[i - 1] + len);
            segmentNormals.push(len > 1e-9 ? { x: -dy / len, y: dx / len } : { x: 0, y: 0 });
          }
          const total = cumulative[cumulative.length - 1];
          const taperLength = total * 0.15;
          const left: Vec2[] = [];
          const right: Vec2[] = [];
          for (let i = 0; i < lane.points.length; i++) {
            const before = i > 0 ? segmentNormals[i - 1] : { x: 0, y: 0 };
            const after = i < segmentNormals.length ? segmentNormals[i] : { x: 0, y: 0 };
            let nx = before.x + after.x;
            let ny = before.y + after.y;
            let nLen = Math.hypot(nx, ny);
            if (nLen <= 1e-9) {
              const fallback = i < segmentNormals.length ? after : before;
              nx = fallback.x;
              ny = fallback.y;
              nLen = Math.hypot(nx, ny);
            }
            if (nLen > 1e-9) {
              nx /= nLen;
              ny /= nLen;
            }
            let halfWidth = lane.half_width;
            if (lane.tapers && taperLength > 0 && total - cumulative[i] < taperLength) {
              const frac = clamp01((total - cumulative[i]) / taperLength);
              halfWidth *= TAPER_FLOOR_FRAC * (1 - frac) + frac;
            }
            const p = lane.points[i];
            left.push({ x: p.x + nx * halfWidth, y: p.y + ny * halfWidth });
            right.push({ x: p.x - nx * halfWidth, y: p.y - ny * halfWidth });
          }
          boundary = { source: lane.points, halfWidth: lane.half_width, tapers: lane.tapers, left, right };
          this.laneBoundaryWorld.set(lane.id, boundary);
        }

        // These two edges mark the FULL swept tolerance where hull coupling,
        // emplacement siting, and junction physics apply. The band inside is
        // still the road; the boundary is never a fill or a wider road.
        for (const edge of [boundary.left, boundary.right]) {
          const edgePts = edge.map((p) => this.worldToScreen(p));
          g.moveTo(edgePts[0].x, edgePts[0].y);
          for (let i = 1; i < edgePts.length; i++) g.lineTo(edgePts[i].x, edgePts[i].y);
          g.stroke({ width: 1, color: 0x2a6fb0, alpha: boundaryAlpha, cap: "round", join: "round" });
        }
      }
      trace();
      g.stroke({ width: widthPx, color: 0x2a6fb0, alpha: 0.075, cap: "round", join: "round" });
      // The centerline: the axis a fleet aligns to for the speed benefit. Kept
      // thin in absolute terms even as the physical corridor fills the view.
      trace();
      const centerlinePx = Math.min(3, Math.max(0.7, widthPx * 0.02));
      g.stroke({ width: centerlinePx, color: 0x6fd0ff, alpha: 0.3, cap: "round" });

      // §comms-v3: green is covered wire; the base blue road stays untouched.
      // Presentation mode uses the relay's separately drawn 2D circle.
      const arc = arcs.get(lane.id)!;
      const zones = commZones.get(lane.id)!;
      const drawZone = (
        intervals: ArcInterval[],
        color: number,
        bandAlpha: number,
        centerAlpha: number,
        widthFrac: number,
      ) => {
        for (const interval of intervals) {
          if (interval.hi - interval.lo < 1e-6) continue;
          const zonePts = laneSpan(arc, interval).map((p) => this.worldToScreen(p));
          g.moveTo(zonePts[0].x, zonePts[0].y);
          for (let i = 1; i < zonePts.length; i++) g.lineTo(zonePts[i].x, zonePts[i].y);
          g.stroke({
            width: Math.max(1.5, widthPx * widthFrac),
            color,
            alpha: bandAlpha,
            cap: "round",
            join: "round",
          });
          g.moveTo(zonePts[0].x, zonePts[0].y);
          for (let i = 1; i < zonePts.length; i++) g.lineTo(zonePts[i].x, zonePts[i].y);
          g.stroke({ width: Math.max(0.8, centerlinePx), color, alpha: centerAlpha, cap: "round" });
        }
      };
      drawZone(zones, COL_ROUTE, 0.11, 0.38, 0.5);
    }
  }

  private drawBackground(): void {
    if (!this.galaxy) return;
    // §perf: pooled Graphics + Text, created once and redrawn in place (was:
    // removeChildren() + new Graphics/Text every time this ran).
    if (!this.bgGfx) {
      this.bgGfx = new Graphics();
      this.hubText = new Text({ text: "HUB", style: new TextStyle({ fill: COL_HUB, fontFamily: "ui-monospace, monospace", fontSize: 10, letterSpacing: 2 }) });
      this.hubText.anchor.set(0.5, 0);
      this.bg.addChild(this.bgGfx, this.hubText);
    }
    const g = this.bgGfx;
    g.clear();
    // (No galaxy radial rings: they implied discrete "zones" that don't exist —
    // radial variation is a CONTINUOUS frontier gradient, not stepped, so the
    // rings marked nothing. The hub landmark stays.)
    const hub = this.worldToScreen(this.galaxy.hub);
    g.circle(hub.x, hub.y, 11).fill({ color: COL_HUB, alpha: 0.18 });
    g.circle(hub.x, hub.y, 6).fill({ color: COL_HUB, alpha: 0.4 });
    g.circle(hub.x, hub.y, 2.5).fill({ color: 0xffffff, alpha: 0.9 });
    this.hubText!.position.set(hub.x, hub.y + 13);
  }

  /// §perf: get-or-create the pooled draw objects for a system id. Created once
  /// and reused across frames (clear()+redraw the Graphics, re-set the texts) so
  /// drawSystems never allocates per frame. Anchors + the constant text styles are
  /// set here; per-frame content/fill/position/alpha are set in drawSystems.
  private ensureSystemGfx(id: string): SystemGfx {
    let e = this.systemGfx.get(id);
    if (!e) {
      const mono = "ui-monospace, monospace";
      e = {
        g: new Graphics(),
        label: new Text({ text: "", style: new TextStyle({ fontFamily: mono, fontSize: 8 }) }),
        blockade: new Text({ text: "", style: new TextStyle({ fill: COL_THREAT, fontFamily: mono, fontSize: 8, fontWeight: "700" }) }),
        enclave: new Text({ text: "", style: new TextStyle({ fill: COL_PIRATE, fontFamily: mono, fontSize: 8, fontWeight: "700" }) }),
        node: new Text({ text: "", style: new TextStyle({ fontFamily: mono, fontSize: 8, fontWeight: "700" }) }),
      };
      e.label.anchor.set(0, 0.5);
      e.blockade.anchor.set(0.5, 1);
      e.enclave.anchor.set(0.5, 1);
      e.node.anchor.set(0.5, 1);
      this.systemGfx.set(id, e);
    }
    return e;
  }

  /// Draw star systems with their resource geology and (light-gated) ownership.
  /// A system's glow grows with its deposit value-rate, so the frontier visibly
  /// out-produces the core (§4); the ring shows ownership — cyan (yours), red (a
  /// rival, once their claim's light has reached you), or dim (unclaimed). Your
  /// own systems also surface their accumulated production.
  ///
  /// §perf: pooled per system (SystemGfx), rebuilt only when geometry is dirty
  /// (camera / new View / selection) OR a system is animating a pulse (rival
  /// breath / blockade). When neither holds the whole layer is skipped — systems
  /// are at fixed world positions, so their screen geometry is unchanged. The
  /// drawn output is identical; only the per-frame object churn is gone.
  private drawSystems(state: ViewState, geomDirty: boolean): void {
    if (!this.galaxy) return;
    if (!geomDirty && !this.systemsAnimating) return;
    const now = performance.now();
    const dynById = new Map(state.systems.map((s) => [s.id, s]));
    const wellT = clamp01((this.scale / this.fitScale() - 8) / (14 - 8));
    const wellAlpha = 0.10 * wellT * wellT * (3 - 2 * wellT);
    // BIG stars first, SMALL stars last (on top): when two systems sit close, a
    // visually larger neighbor must never bury a small star — its home ring and
    // disk stay visible and aimable (nearest-center picking then just works).
    const ordered = [...this.galaxy.systems]
      .sort((a, b) => this.starDiameters(b).rendered - this.starDiameters(a).rendered);
    let animating = false;
    // Detach the pooled children (NOT destroyed), re-added in sort order below so
    // the z-order (and any zoom-driven re-sort) stays exactly as before.
    this.systemsLayer.removeChildren();
    for (const sys of ordered) {
      const e = this.ensureSystemGfx(sys.id);
      const g = e.g;
      g.clear();
      e.label.visible = true;
      e.blockade.visible = false;
      e.enclave.visible = false;
      e.node.visible = false;
      const s = this.worldToScreen(sys.pos);
      const dyn = dynById.get(sys.id);
      const owner = dyn?.owner ?? null;
      const mine = owner !== null && owner === state.playerId;
      // §syndicates: an ALLY-owned system (per the viewer's light-delayed
      // knowledge) tints friendly-green; a plain rival stays red.
      const ally = owner !== null && !mine && !!dyn?.ally;
      const rival = owner !== null && !mine && !ally;
      const selected = state.selectedSystemId === sys.id;

      // §explore: BAND → glow size (the public gradient made visible — 3 steps).
      // Neutral tint: the dominant-resource color was exact-geology knowledge.
      const glow = BAND_GLOW[sys.band] ?? BAND_GLOW.poor;
      const topColor = COL_SYSTEM;

      // §size-hierarchy: the star's rendered VISIBLE diameter — its normal-zoom
      // deposit-value size through r=72, then the body curve grows it over the
      // final approach (see starDiameters). Every ownership ring /
      // halo / label below keeps its ORIGINAL radius plus only `extra` (the
      // radius the disk gained in the deep-zoom band) — so normal zoom is
      // pixel-identical to before, and at deep zoom the cues ride out with the
      // growing rim instead of drowning inside the giant disk.
      const { base: bodyD, rendered } = this.starDiameters(sys);
      const extra = (rendered - bodyD) / 2;

      // The IMPULSE boundary: inside this world-anchored well no drive lights.
      // Every public system is in the sim's well set; chart zoom stays clean.
      if (wellAlpha >= 0.005) {
        g.circle(s.x, s.y, HYPERLIMIT_SU * this.scale)
          .stroke({ width: 1, color: COL_IMPULSE_BOUNDARY, alpha: wellAlpha });
      }

      g.circle(s.x, s.y, glow).fill({ color: topColor, alpha: 0.07 }); // geology value-glow

      // Ownership treatment — own and rival are a matched pair (halo + bold ring),
      // so territory reads at a glance; unclaimed systems stay deliberately subdued
      // (no ring) so they recede. Ownership is still light-gated upstream: a rival
      // only appears as rival once their claim's light has reached this player.
      if (mine) {
        // Friendly territory: cyan halo + bold ring.
        g.circle(s.x, s.y, 10 + extra).fill({ color: COL_OWN, alpha: 0.10 });
        g.circle(s.x, s.y, 7 + extra).stroke({ width: 1.8, color: COL_OWN, alpha: 0.95 });
      } else if (ally) {
        // §syndicates: ally territory — a green halo + bold ring, the friendly
        // treatment in a distinct hue (no rival danger-breath).
        g.circle(s.x, s.y, 10 + extra).fill({ color: COL_ALLY, alpha: 0.10 });
        g.circle(s.x, s.y, 7 + extra).stroke({ width: 1.8, color: COL_ALLY, alpha: 0.9 });
      } else if (rival) {
        // Rival / contested territory: a slow-breathing red danger halo + a bold
        // DOUBLE ring — unmistakable as hostile-held, and clearly distinct from the
        // fast-pulsing raider-threat marker (slower cadence, static rings, sized to
        // the system body, softer COL_OTHER hue vs. the alert COL_THREAT red).
        const breath = 0.5 + 0.5 * Math.sin(now / 1100);
        g.circle(s.x, s.y, 13 + extra).fill({ color: COL_OTHER, alpha: 0.05 + 0.07 * breath });
        g.circle(s.x, s.y, 9.5 + extra).stroke({ width: 1, color: COL_OTHER, alpha: 0.4 });
        g.circle(s.x, s.y, 7 + extra).stroke({ width: 2, color: COL_OTHER, alpha: 0.98 });
        animating = true; // the breath halo pulses every frame
      }
      if (selected) {
        g.circle(s.x, s.y, (owner !== null ? 12 : glow + 4) + extra).stroke({ width: 1.2, color: 0xffffff, alpha: 0.85 });
      }
      // The BODY itself: the system's assigned STAR-TYPE icon (deterministic by id,
      // stars.ts), pooled, sized by deposit value (the frontier-richer hierarchy)
      // and dimmed when unclaimed so owned/rival territory leads. The glow +
      // ownership rings + label above are the data cues; the star is just the body
      // they decorate — ownership stays on the RING, and the star icon carries NO
      // tint, so a blue star is never mistaken for "owned" nor a red star for
      // "rival". Dot fallback until the icon loads. Because each icon's VISIBLE star
      // fills a different area of its transparent canvas, use the type's manifest
      // `center`/`visualDiameter` to CENTRE the visible star at the system and size
      // that visible disk (not the canvas) to bodyD — so every type reads at a
      // consistent on-map size regardless of its icon's fill.
      const st = starTypeFor(sys.id);
      const starTex = this.starTex.get(st.slug);
      if (starTex) {
        const bsp = this.bodyFor(sys.id, starTex);
        const anchor = starAnchor(st);
        bsp.anchor.set(anchor[0], anchor[1]);
        bsp.position.set(s.x, s.y);
        bsp.scale.set(rendered / (starVisualRatio(st) * starTex.width));
        // Keep unclaimed stars near-full brightness so the vivid star art reads
        // (ownership is carried by the RING, not by dimming the star); owned/rival
        // still lead via their full brightness + ring.
        bsp.alpha = owner !== null ? 1 : 0.9;
      } else {
        const dotCol = mine ? COL_OWN : ally ? COL_ALLY : rival ? COL_OTHER : COL_SYSTEM;
        g.circle(s.x, s.y, 2.4).fill({ color: dotCol, alpha: 0.95 });
      }

      // Label: name; your own systems also show their top stockpiled good.
      let txt = sys.name;
      if (mine && dyn?.stockpile && dyn.stockpile.length) {
        const top = dyn.stockpile.reduce((a, b) => (a.units > b.units ? a : b));
        txt = `${sys.name}  ◆${top.units} ${label(top.commodity)}`;
      }
      const col = mine ? COL_OWN : ally ? COL_ALLY : rival ? COL_OTHER : 0x55657f;
      const t = e.label;
      t.text = txt;
      t.style.fill = col;
      t.position.set(s.x + glow + 2 + extra, s.y); // +extra: rides the grown rim at deep zoom
      t.alpha = mine ? 0.95 : ally ? 0.9 : rival ? 0.88 : selected ? 0.8 : 0.5;

      // §contestable-territory Part 1: a BLOCKADE marker — a slow-pulsing red
      // dashed ring around a besieged system + a "⛔ BLOCKADE" tag. Participant-
      // only (the view field is fog-gated), so it draws for the owner (their
      // system besieged) and the besieger (their blockade), never a third party.
      if (dyn?.blockade) {
        const half = rendered / 2;
        const pulse = 0.5 + 0.5 * Math.sin(now / 500);
        const rr = Math.max(15, half + 6) + extra;
        const seg = 22;
        for (let i = 0; i < seg; i += 2) {
          const a0 = (i / seg) * Math.PI * 2;
          const a1 = ((i + 1) / seg) * Math.PI * 2;
          g.moveTo(s.x + Math.cos(a0) * rr, s.y + Math.sin(a0) * rr)
            .lineTo(s.x + Math.cos(a1) * rr, s.y + Math.sin(a1) * rr);
        }
        g.stroke({ width: 1.6, color: COL_THREAT, alpha: 0.4 + 0.4 * pulse });
        const bt = e.blockade;
        bt.text = dyn.blockade.by_me ? "⛔ BLOCKADING" : "⛔ BLOCKADE";
        bt.position.set(s.x, s.y - rr - 2);
        bt.alpha = 0.7 + 0.3 * pulse;
        bt.visible = true;
        animating = true; // the blockade ring + tag pulse every frame
      }
      // §pirates: a SCOUTED enclave base — an amber dashed ring + "☠ ENCLAVE T‹n›"
      // tag. Owner-only knowledge (your own scout snapshot); the base is DARK to
      // anyone who hasn't scouted it (the intel field is fog-gated in the View).
      if (dyn?.intel && (dyn.intel.enclave_tier ?? 0) > 0) {
        const half = rendered / 2;
        const rr = Math.max(14, half + 5) + extra;
        const seg = 20;
        for (let i = 0; i < seg; i += 2) {
          const a0 = (i / seg) * Math.PI * 2;
          const a1 = ((i + 1) / seg) * Math.PI * 2;
          g.moveTo(s.x + Math.cos(a0) * rr, s.y + Math.sin(a0) * rr)
            .lineTo(s.x + Math.cos(a1) * rr, s.y + Math.sin(a1) * rr);
        }
        g.stroke({ width: 1.4, color: COL_PIRATE, alpha: 0.7 });
        const pt = e.enclave;
        pt.text = `☠ ENCLAVE T${dyn.intel.enclave_tier}`;
        pt.position.set(s.x, s.y - rr - 2);
        pt.alpha = 0.9;
        pt.visible = true;
      }
      // §node: an EXOTIC NODE badge. DORMANT before the awakening time → a dim "◈"
      // telegraph so players see WHERE nodes will awaken from t=0. AWAKENED → a
      // violet ring + the bonus title, tinted toward the holder (mine/ally/rival)
      // so the map reads who commands it. The HOLDER also gets a faint REGION RING
      // (radius sent owner-only) — solid when the bonus is live, dashed when the
      // node is UNFED (bonus suspended).
      const nd = dyn?.node;
      if (nd) {
        const half = rendered / 2;
        const holderCol = mine ? COL_OWN : ally ? COL_ALLY : rival ? COL_OTHER : COL_NODE;
        if (!nd.awakened) {
          const gl = e.node;
          gl.text = "◈";
          gl.style.fill = COL_NODE;
          gl.style.fontSize = 9;
          gl.position.set(s.x, s.y - Math.max(12, half + 4) - extra);
          gl.alpha = 0.45;
          gl.visible = true;
        } else {
          const rr = Math.max(16, half + 7) + extra;
          g.circle(s.x, s.y, rr).stroke({ width: 1.5, color: COL_NODE, alpha: 0.8 });
          const nt = e.node;
          nt.text = `◈ ${nd.title.toUpperCase()}`;
          nt.style.fill = holderCol;
          nt.style.fontSize = 8;
          nt.position.set(s.x, s.y - rr - 2);
          nt.alpha = 0.95;
          nt.visible = true;
          // HOLDER-ONLY region ring (region_radius > 0 only for the owner).
          if (nd.region_radius > 0) {
            const reg = nd.region_radius * this.scale;
            if (nd.fed) {
              g.circle(s.x, s.y, reg).stroke({ width: 1, color: COL_NODE, alpha: 0.16 });
            } else {
              const seg = 48;
              for (let i = 0; i < seg; i += 2) {
                const a0 = (i / seg) * Math.PI * 2;
                const a1 = ((i + 1) / seg) * Math.PI * 2;
                g.moveTo(s.x + Math.cos(a0) * reg, s.y + Math.sin(a0) * reg)
                  .lineTo(s.x + Math.cos(a1) * reg, s.y + Math.sin(a1) * reg);
              }
              g.stroke({ width: 1, color: COL_NODE, alpha: 0.12 });
            }
          }
        }
      }
      // Re-attach in sort order (big → small): the geometry, then the label, then
      // the three optional tags. Invisible tags render nothing.
      this.systemsLayer.addChild(g, e.label, e.blockade, e.enclave, e.node);
    }
    this.systemsAnimating = animating;
  }

  /// §size-hierarchy: a system's star VISIBLE diameter — `base` at normal zoom
  /// (the deposit-value 20–46px, unchanged) and `rendered` at the current zoom
  /// (the late body-bloom curve). One place computes both so the body sprite,
  /// its ownership rings/label, and the click hit-test all agree.
  /// The deep-zoom endpoint is the EXISTING System View star's visible diameter.
  /// This is deliberately a visible-disk cap rather than a canvas cap: texture
  /// fill ratios differ, but no galaxy star may outgrow its unchanged schematic
  /// counterpart. Both layers therefore meet at one equal-size handoff anchor.
  private starDiameters(sys: SystemInfo): { base: number; rendered: number } {
    // §explore: the public BAND drives the size — 3 steps within the old 20–46px
    // range (public info, identical for every viewer; no geology leak).
    const systemDiameter = this.systemScene.starVisibleDiameterPx();
    const base = Math.min(BAND_SIZE[sys.band] ?? BAND_SIZE.poor, systemDiameter); // target VISIBLE diameter, normal zoom
    return { base, rendered: this.deepZoomPx(base, systemDiameter, BODY_ZOOM_START, ZOOM_MAX_FACTOR) };
  }

  /// A system's click hit radius: half its rendered disk, capped so a max-zoom
  /// giant never swallows clicks meant for the fleets parked on it (ships are
  /// hit-tested first in main.ts and stay well under the cap). Floored by the
  /// caller (main.ts keeps its old 15px minimum for normal zoom).
  systemHitRadius(sys: SystemInfo): number {
    return Math.min(this.starDiameters(sys).rendered / 2, BODY_HIT_CAP_PX);
  }

  private drawAnchors(state: ViewState): void {
    // §perf: pooled anchorGfx (clear+redraw) + a single pooled HOME text, instead
    // of removeChildren()+new Graphics/Text every frame. A corp has exactly one
    // command seat, so one HOME text is repositioned and shown/hidden.
    const g = this.anchorGfx;
    g.clear();
    let homeShown = false;
    if (this.galaxy) {
      for (const a of state.anchors) {
        const own = a.owner !== null && a.owner === state.playerId;
        const s = this.worldToScreen(a.pos);
        // A command base now coincides with the owner's HOME STAR SYSTEM, which is
        // drawn as an owned cyan/red system (+ the command-center pulse for your own).
        // So skip the redundant anchor circle when a system sits here — no more
        // "mystery circle." Only draw a glyph for a base in OPEN space (e.g. a
        // command center relocated away from its home system, a future mechanic).
        const atSystem = this.galaxy.systems.some(
          (sys) => Math.abs(sys.pos.x - a.pos.x) < 1 && Math.abs(sys.pos.y - a.pos.y) < 1,
        );
        if (!atSystem) {
          const color = own ? COL_ANCHOR_OWN : COL_ANCHOR_OTHER;
          if (a.owner) {
            g.circle(s.x, s.y, own ? 9 : 6).fill({ color, alpha: own ? 0.22 : 0.14 });
            g.circle(s.x, s.y, 3).fill({ color, alpha: 0.9 });
          } else {
            g.circle(s.x, s.y, 4).stroke({ width: 1, color: 0x3a4660, alpha: 0.7 });
          }
        }
        // Name your own command seat "HOME" (above the home system's own label —
        // riding the star's rendered rim, so it clears the grown disk at deep zoom).
        if (own && this.homeText) {
          const homeSys = this.galaxy.systems.find(
            (sys) => Math.abs(sys.pos.x - a.pos.x) < 1 && Math.abs(sys.pos.y - a.pos.y) < 1,
          );
          const dm = homeSys ? this.starDiameters(homeSys) : null;
          const extra = dm ? (dm.rendered - dm.base) / 2 : 0; // deep-zoom growth only — normal zoom identical
          this.homeText.position.set(s.x, s.y - 13 - extra);
          this.homeText.visible = true;
          homeShown = true;
        }
      }
    }
    if (this.homeText && !homeShown) this.homeText.visible = false;
  }

  /// Soft, fuzzy INTERCEPT ESTIMATES for committed raids (§8, §14.1). A CRUDE
  /// constant-velocity lead projection from the delayed ghosts — honest, since
  /// the real pursuit acts on light-delayed sightings it hasn't seen yet, so it
  /// is EXPECTED to drift. Rendered in the sensor-circle idiom
  /// (translucent, soft, concentric) precisely so it reads as "best guess, about
  /// here," the way a sensor circle reads as a soft boundary — honest uncertainty,
  /// not a precise promise.
  private drawIntercepts(state: ViewState, ghostById: Map<string, GhostView>): void {
    const g = this.interceptGfx;
    g.clear();
    const live = new Set<string>();
    if (this.galaxy) {
      const raiderSpeed = Math.max(this.galaxy.raider_speed || 100, 1);
      for (const [raiderId, targetId] of Object.entries(state.raids)) {
        // §perf: O(1) lookup from the shared ghost map (was two O(n) find()s per
        // raid per frame).
        const r = ghostById.get(raiderId);
        const t = ghostById.get(targetId);
        if (!r || !t) continue; // a ship left the view — no guess to draw
        if (ghostInTunnel(r) || ghostInTunnel(t)) continue; // intent may remain; position-derived estimates may not

        // Constant-velocity intercept: ETA ≈ range / cruise speed (§14.1, no
        // acceleration ramp), then project the target forward along its heading.
        const range = Math.hypot(t.pos.x - r.pos.x, t.pos.y - r.pos.y);
        const eta = range / raiderSpeed;
        const ip = { x: t.pos.x + t.vel.x * eta, y: t.pos.y + t.vel.y * eta };
        const s = this.worldToScreen(ip);
        const rp = this.worldToScreen({ x: r.pos.x, y: r.pos.y });

        // Fuzzier the farther out (more uncertain). Soft fill + faint concentric
        // rings = the "approximate zone" idiom.
        const rad = Math.min(12 + eta * 1.4, 48);
        g.circle(s.x, s.y, rad).fill({ color: COL_ESTIMATE, alpha: 0.05 });
        for (const f of [1.0, 0.66, 0.34]) {
          g.circle(s.x, s.y, rad * f).stroke({ width: 1, color: COL_ESTIMATE, alpha: 0.1 + (1 - f) * 0.08 });
        }
        g.circle(s.x, s.y, 1.6).fill({ color: COL_ESTIMATE, alpha: 0.5 });
        // Faint dashed guidance from the raider to the estimate (not a path).
        dashedLine(g, rp.x, rp.y, s.x, s.y, 4, 10);
        g.stroke({ width: 1, color: COL_ESTIMATE, alpha: 0.12 });

        const label = this.interceptLabel(raiderId);
        label.text = `≈ intercept · ~${Math.round(eta)}s`;
        label.position.set(s.x + rad + 3, s.y);
        label.visible = true;
        live.add(raiderId);
      }
    }
    // §perf: destroy + drop stale intercept labels (was: hidden forever, so the
    // map + their text textures grew unboundedly over a session). The lazy getter
    // recreates one on demand the next time that raider commits a raid.
    for (const [id, label] of this.interceptLabels) {
      if (!live.has(id)) {
        this.signalsLayer.removeChild(label);
        label.destroy();
        this.interceptLabels.delete(id);
      }
    }
  }

  /// §battles-take-time: a pulsing BATTLE MARKER at each ongoing engagement the
  /// player can see (strictly light-gated by the server) — the "battle in
  /// progress" icon when its art is loaded (same pulse cadence), the original
  /// drawn burst otherwise. Under the ghosts — "something is happening HERE".
  private drawBattles(state: ViewState): void {
    const g = this.interceptGfx;
    const now = performance.now();
    const pulse = 0.5 + 0.5 * Math.sin(now / 200);
    this.battleHits = [];
    const live = new Set<string>();
    // §one-battle-one-icon: two SEPARATE engagements whose anchors nearly
    // coincide fan out slightly so they stay two icons (a merged fight is one
    // engagement id → one icon already).
    const slotByCell = new Map<string, number>();
    for (const b of state.battles) {
      const base = this.worldToScreen(b.pos);
      const cell = `${Math.round(b.pos.x / 50)},${Math.round(b.pos.y / 50)}`;
      const slot = slotByCell.get(cell) ?? 0;
      slotByCell.set(cell, slot + 1);
      const sx = base.x + slot * (BATTLE_ONGOING_PX * 0.85);
      const sy = base.y - slot * 4;
      live.add(b.id);
      if (this.texBattleOngoing) {
        let sp = this.battleSprites.get(b.id);
        if (!sp) {
          sp = new Sprite(this.texBattleOngoing);
          sp.anchor.set(0.5);
          this.aftermathLayer.addChild(sp);
          this.battleSprites.set(b.id, sp);
        }
        sp.visible = true;
        sp.texture = this.texBattleOngoing;
        sp.position.set(sx, sy);
        sp.scale.set(((BATTLE_ONGOING_PX + pulse * 5) / this.texBattleOngoing.width));
        sp.alpha = 0.7 + 0.3 * pulse;
        // Keep the alert ring so the icon still SHOUTS like the old burst did.
        g.circle(sx, sy, BATTLE_ONGOING_PX * 0.7 + pulse * 5).stroke({ width: 1.4, color: COL_THREAT, alpha: 0.25 + 0.35 * pulse });
      } else {
        const r = 14 + pulse * 6;
        for (let i = 0; i < 8; i++) {
          const a = (i / 8) * Math.PI * 2 + now / 1400;
          g.moveTo(sx + Math.cos(a) * r * 0.5, sy + Math.sin(a) * r * 0.5).lineTo(sx + Math.cos(a) * r, sy + Math.sin(a) * r);
        }
        g.stroke({ width: 1.5, color: COL_THREAT, alpha: 0.35 + 0.4 * pulse });
        g.circle(sx, sy, 3.2).fill({ color: COL_THREAT, alpha: 0.75 });
      }
      // OWN-INVOLVEMENT PIP: one cyan diamond on the icon's edge if the viewer
      // has forces in this fight — "my fight" at a glance (one pip regardless of
      // how many of their fleets are in). No rival pips beyond the site-reveal.
      if (b.own) {
        const pr = 4;
        const px = sx + BATTLE_ONGOING_PX * 0.42;
        const py = sy - BATTLE_ONGOING_PX * 0.42;
        const diamond = (rr: number): number[] => [px, py - rr, px + rr, py, px, py + rr, px - rr, py];
        g.poly(diamond(pr + 1.3)).fill({ color: 0x05070d, alpha: 0.8 });
        g.poly(diamond(pr)).fill({ color: COL_OWN, alpha: 0.95 });
      }
      this.battleHits.push({ id: b.id, sx, sy });
    }
    // Destroy pooled icons for engagements that have ended.
    for (const [id, sp] of this.battleSprites) {
      if (!live.has(id)) {
        sp.destroy();
        this.battleSprites.delete(id);
      }
    }
  }

  /// Hit-test the ongoing-battle icons (screen-space, fixed radius). Returns the
  /// clicked engagement id, or null. Consumed by main.ts's map click.
  battlePick(sx: number, sy: number): string | null {
    let best: string | null = null;
    let bestD = BATTLE_ONGOING_PX * 0.65;
    for (const h of this.battleHits) {
      const d = Math.hypot(h.sx - sx, h.sy - sy);
      if (d < bestD) { bestD = d; best = h.id; }
    }
    return best;
  }

  /// §battle-aftermath: the concluded-battle markers — one per RETAINED report
  /// (owner-only by construction: the server only sends reports you were in,
  /// and each appears only once YOUR conclusion light arrived). SCREEN-SPACE
  /// UI like pips/badges: fixed size at every zoom, never in the deep-zoom
  /// ramp. Unviewed = subtle attention pulse; viewed = static + dimmed;
  /// dismissed / older than BATTLE_MARKER_TTL_S = hidden. Co-located battles
  /// fan out in a small row so each stays clickable.
  private drawAftermath(state: ViewState): void {
    const g = this.aftermathGfx;
    g.clear();
    this.aftermathHits = [];
    const simNow = liveSimTime();
    const live = new Set<number>();
    const slotIndex = new Map<string, number>();
    for (const r of state.battleReports) {
      if (state.battleDismissed.has(r.id)) continue;
      // An ESCAPED raid isn't a battle — no contact, no wreckage — so it leaves no
      // aftermath marker on the map (the "raid failed" news still lands in the log).
      if (r.outcome === "escaped") continue;
      if (simNow - r.learned_at > BATTLE_MARKER_TTL_S) continue;
      const s = this.worldToScreen(r.pos);
      const key = `${Math.round(r.pos.x / 60)},${Math.round(r.pos.y / 60)}`;
      const slot = slotIndex.get(key) ?? 0;
      slotIndex.set(key, slot + 1);
      const sx = s.x + slot * (BATTLE_MARKER_PX * 0.7);
      const sy = s.y - slot * 4;
      const viewed = state.battleViewed.has(r.id);
      const pulse = viewed ? 0 : 0.5 + 0.5 * Math.sin(performance.now() / 320);
      // A VIEWED marker is static (no pulse) and dimmer — history, not a live
      // alert — but the aftermath art is a low-saturation cool grey-teal, so a
      // very low alpha reads as "greyed out / broken." Keep it clearly legible.
      const base = viewed ? 0.68 : 0.8 + 0.2 * pulse;
      // §aftermath-fade: decay the whole marker toward the floor as its report ages.
      const alpha = this.aftermathFadeAlpha(base, simNow - r.learned_at);
      live.add(r.id);
      if (r.id === this.selectedBattleMarkerId) this.drawMarkerSelectionRing(g, sx, sy);
      if (this.texBattleAftermath) {
        let sp = this.aftermathSprites.get(r.id);
        if (!sp) {
          sp = new Sprite(this.texBattleAftermath);
          sp.anchor.set(0.5);
          this.aftermathLayer.addChild(sp);
          this.aftermathSprites.set(r.id, sp);
        }
        sp.position.set(sx, sy);
        sp.scale.set(BATTLE_MARKER_PX / this.texBattleAftermath.width);
        sp.alpha = alpha;
      } else {
        // Drawn fallback (used only if battle_aftermath.png fails to load): a
        // broken-blade cross + drifting-debris arc, in a cooled-ember tone
        // (this is HISTORY, not the red alert of an ongoing battle).
        const col = viewed ? 0x8a8f9c : 0xd08a5a;
        const r2 = BATTLE_MARKER_PX * 0.32;
        g.moveTo(sx - r2, sy - r2).lineTo(sx + r2 * 0.4, sy + r2 * 0.4).stroke({ width: 1.8, color: col, alpha });
        g.moveTo(sx + r2, sy - r2).lineTo(sx - r2 * 0.4, sy + r2 * 0.4).stroke({ width: 1.8, color: col, alpha });
        g.arc(sx, sy, r2 * 1.7, -Math.PI * 0.15, Math.PI * 0.45).stroke({ width: 1, color: col, alpha: alpha * 0.7 });
        g.circle(sx + r2 * 1.5, sy + r2 * 0.9, 1.1).fill({ color: col, alpha });
      }
      if (!viewed) {
        // The new-report attention pulse (subtle — an invitation, not an alarm).
        // Fades with the marker so an old, never-opened report goes quiet too.
        g.circle(sx, sy, BATTLE_MARKER_PX * 0.7 + pulse * 3).stroke({ width: 1, color: 0xd08a5a, alpha: alpha * (0.2 + 0.4 * pulse) });
      }
      this.aftermathHits.push({ id: r.id, sx, sy });
    }
    for (const [id, sp] of this.aftermathSprites) {
      if (!live.has(id)) {
        sp.destroy();
        this.aftermathSprites.delete(id);
      }
    }
  }

  /// §aftermath-fade: the marker's alpha given how long ago the viewer's report
  /// arrived. `base` (fresh alpha) at age 0, a smoothstep decay over
  /// AFTERMATH_FADE_SECS, then held at AFTERMATH_FLOOR_ALPHA until the TTL prunes
  /// it. Monotonic — a marker only ever gets dimmer as it ages into the dark.
  private aftermathFadeAlpha(base: number, ageSecs: number): number {
    const t = Math.min(1, Math.max(0, ageSecs / AFTERMATH_FADE_SECS));
    const smooth = t * t * (3 - 2 * t); // smoothstep for a soft fade, not a linear ramp
    return AFTERMATH_FLOOR_ALPHA + (base - AFTERMATH_FLOOR_ALPHA) * (1 - smooth);
  }

  /// The STANDARD selection ring (matches a selected system: thin, white) around a
  /// concluded-battle marker. Drawn at full alpha regardless of the marker's fade,
  /// so a nearly-dark marker still reads as selected and stays reachable.
  private drawMarkerSelectionRing(g: Graphics, sx: number, sy: number): void {
    g.circle(sx, sy, BATTLE_MARKER_PX * 0.62).stroke({ width: 1.2, color: 0xffffff, alpha: 0.85 });
  }

  /// Hit-test the aftermath markers (screen-space, fixed radius). Returns the
  /// clicked report id, or null. Consumed by main.ts's map click.
  aftermathPick(sx: number, sy: number): number | null {
    let best: number | null = null;
    let bestD = BATTLE_MARKER_HIT_PX;
    for (const h of this.aftermathHits) {
      const d = Math.hypot(h.sx - sx, h.sy - sy);
      if (d < bestD) {
        bestD = d;
        best = h.id;
      }
    }
    return best;
  }

  /// §contestable-territory Part 2: CAPTURE markers — a flip changed a system's
  /// hands. Screen-space UI like the aftermath markers (fixed size, never grows),
  /// under the ghosts. A GOLD flag = you captured; RED = you lost. Unviewed
  /// pulses; viewed dims; dismissed / older than the TTL are hidden. Shares the
  /// battleViewed / battleDismissed sets with battles (ids are globally unique).
  private drawCaptures(state: ViewState): void {
    const g = this.aftermathGfx; // same layer as the aftermath vector fallback
    this.captureHits = [];
    const simNow = liveSimTime();
    for (const r of state.captureReports) {
      if (state.battleDismissed.has(r.id)) continue;
      if (simNow - r.learned_at > BATTLE_MARKER_TTL_S) continue;
      const s = this.worldToScreen(r.pos);
      const viewed = state.battleViewed.has(r.id);
      const pulse = viewed ? 0 : 0.5 + 0.5 * Math.sin(performance.now() / 320);
      const base = viewed ? 0.68 : 0.8 + 0.2 * pulse; // legible-when-viewed (see drawAftermath)
      const alpha = this.aftermathFadeAlpha(base, simNow - r.learned_at); // §aftermath-fade
      const col = r.captor ? 0xffcf6b : COL_THREAT; // gold = gained, red = lost
      // A little flag on a pole (territory changed hands).
      const px = s.x;
      const py = s.y;
      const h = BATTLE_MARKER_PX * 0.5;
      if (r.id === this.selectedBattleMarkerId) this.drawMarkerSelectionRing(g, px, py - h * 0.4);
      g.moveTo(px, py + h * 0.6).lineTo(px, py - h).stroke({ width: 1.6, color: col, alpha });
      g.poly([px, py - h, px + h * 0.9, py - h * 0.6, px, py - h * 0.2]).fill({ color: col, alpha });
      if (!viewed) {
        g.circle(px, py - h * 0.4, BATTLE_MARKER_PX * 0.7 + pulse * 3).stroke({ width: 1, color: col, alpha: alpha * (0.2 + 0.4 * pulse) });
      }
      this.captureHits.push({ id: r.id, sx: px, sy: py });
    }
  }

  /// Hit-test the capture markers (screen-space, fixed radius). Consumed by
  /// main.ts's map click, checked alongside the aftermath markers.
  capturePick(sx: number, sy: number): number | null {
    let best: number | null = null;
    let bestD = BATTLE_MARKER_HIT_PX;
    for (const h of this.captureHits) {
      const d = Math.hypot(h.sx - sx, h.sy - sy);
      if (d < bestD) { bestD = d; best = h.id; }
    }
    return best;
  }

  private interceptLabel(id: string): Text {
    let t = this.interceptLabels.get(id);
    if (!t) {
      t = new Text({
        text: "",
        style: new TextStyle({ fill: COL_ESTIMATE, fontFamily: "ui-monospace, monospace", fontSize: 9, letterSpacing: 0.5 }),
      });
      t.anchor.set(0, 0.5);
      t.alpha = 0.8;
      this.signalsLayer.addChild(t);
      this.interceptLabels.set(id, t);
    }
    return t;
  }

  private commandSignalLabel(id: number): Text {
    let t = this.commandSignalLabels.get(id);
    if (!t) {
      t = new Text({
        text: "beyond comms · warp speed",
        style: new TextStyle({ fill: COL_COMMAND, fontFamily: "ui-monospace, monospace", fontSize: 9, letterSpacing: 0.3 }),
      });
      t.anchor.set(0, 0.5);
      this.signalsLayer.addChild(t);
      this.commandSignalLabels.set(id, t);
    }
    return t;
  }

  /// The command center: the player's vantage, with a pulsing ring.
  private drawCommandCenter(state: ViewState): void {
    // §perf: pooled ccGfx (clear+redraw). Runs every frame — the pulse animates.
    const g = this.ccGfx;
    g.clear();
    if (!state.commandCenter) return;
    const s = this.worldToScreen(state.commandCenter);
    const pulse = 0.5 + 0.5 * Math.sin(performance.now() / 600);
    g.circle(s.x, s.y, 14 + pulse * 4).stroke({ width: 1, color: COL_OWN, alpha: 0.25 + 0.25 * pulse });
    g.circle(s.x, s.y, 5).stroke({ width: 1.5, color: COL_OWN, alpha: 0.9 });
  }


  /// Convoy broadcast routes: because convoys broadcast position + heading, show
  /// their waypoints and the path between them (light-delayed like the rest).
  private drawRoutes(state: ViewState): void {
    const g = this.routesGfx;
    g.clear();
    for (const gh of state.ghosts) {
      if (gh.kind !== "convoy" || !gh.route || gh.route.length < 1) continue;
      const color = gh.own ? COL_OWN : gh.tca ? COL_TCA : gh.pirate ? COL_PIRATE : gh.ally ? COL_ALLY : COL_OTHER;
      const pts = gh.route.map((w) => this.worldToScreen(w));
      g.moveTo(pts[0].x, pts[0].y);
      for (let i = 1; i < pts.length; i++) g.lineTo(pts[i].x, pts[i].y);
      if (pts.length > 2) g.lineTo(pts[0].x, pts[0].y); // close the patrol loop
      g.stroke({ width: 1, color, alpha: 0.2 });
      for (const p of pts) g.circle(p.x, p.y, 2.4).stroke({ width: 1, color, alpha: 0.45 });
    }
  }

  private drawOrders(state: ViewState, drawableFleetIds: Set<string>): void {
    // §perf: one pooled Graphics, cleared + redrawn (was: a new Graphics per order
    // per frame). Each order commits its own path via stroke(), so the combined
    // output is identical. Runs every frame — order lines follow the moving ghosts.
    const g = this.orderGfx;
    g.clear();
    // A pending intent is static, owner-only PLAN geometry rooted in the last
    // served sighting. It remains until a served sample at/after delivery makes
    // the fleet's actual flight plan observable; it never advances a marker.
    const pendingIntents = [...state.pendingOrders.values()]
      .flat()
      .filter((order) => {
        if (order.lost) return false;
        if ((order.intent_path?.length ?? 0) < 2) return false;
        const ghost = state.ghosts.find((candidate) => candidate.id === order.fleet_id && candidate.own);
        return !!ghost && state.simTime - ghost.age < order.arrives_at;
      });
    const fleetsWithPendingIntent = new Set(pendingIntents.map((order) => order.fleet_id));
    // Keep client-issued routes visible as before, and also show the selected
    // fleet's served plan after a reload (when the client-local `orders` map is
    // empty). The plan itself is the information source in that case.
    const currentIds = new Set(Object.keys(state.orders));
    for (const [shipId, bookmark] of Object.entries(state.tunnelBookmarks)) {
      if (bookmark.path?.length) currentIds.add(shipId);
    }
    const selected = state.selectedShipId
      ? state.ghosts.find((ghost) => ghost.id === state.selectedShipId && ghost.own)
      : undefined;
    if (selected?.path?.length) currentIds.add(selected.id);
    for (const shipId of currentIds) {
      // Engaged and docked fleets move to other map surfaces, but a tunnel
      // fleet deliberately keeps its plan beside the stationary bookmark.
      // Never use an eased glyph as route information; this line belongs
      // to the served sighting and freezes whenever that sighting does.
      if (!drawableFleetIds.has(shipId)) continue;
      const ghost = state.ghosts.find((x) => x.id === shipId);
      if (!ghost) continue;
      // A tunnel's served replay keeps receiving old light, but v4 deliberately
      // presents only the stationary entry bookmark. Route geometry is part of
      // that same picture: consuming it from a newer hidden-transit sample would
      // leak motion by making the line shrink behind an unmoving arrow.
      const tunnelBookmark = ghostInTunnel(ghost) ? state.tunnelBookmarks[ghost.id] : undefined;
      const routeAnchor = tunnelBookmark?.pos ?? ghost.pos;
      const from = this.worldToScreen(routeAnchor);
      const dest = state.orders[shipId] ?? ghost.path?.[ghost.path.length - 1]?.pos;
      // §course-plan: draw the flight the sim is ACTUALLY flying when we know
      // it — the ship's remaining legs, lane rides bright and solid, warp hops
      // dashed. The straight line was a lie whenever the plan rode a lane: the
      // ship visibly left it, and the map looked broken rather than clever.
      // Fallback to the straight dashed line while the order is still in
      // flight to the ship (no plan exists yet — honestly so).
      const plan = ghost?.own ? tunnelBookmark?.path ?? ghost.path : null;
      if (plan && plan.length > 0) {
        // The server intentionally serves the plan that was in force at this
        // sighting without trimming flown waypoints. Consume it from the map's
        // served anchor—not server truth and not an eased glyph. In a tunnel
        // that anchor is the frozen bookmark, so no newer replay leaks motion.
        const projection = nearestOnPath(routeAnchor, plan);
        let prev = this.worldToScreen(projection);
        for (let i = projection.next; i < plan.length; i++) {
          const leg = plan[i];
          const p = this.worldToScreen(leg.pos);
          if (leg.lane) {
            g.moveTo(prev.x, prev.y).lineTo(p.x, p.y);
            g.stroke({ width: 1.5, color: COL_ROUTE, alpha: 0.6 });
          } else {
            dashedLine(g, prev.x, prev.y, p.x, p.y, 6, 5);
            g.stroke({ width: 1, color: COL_ROUTE, alpha: 0.45 });
          }
          prev = p;
        }
        g.circle(prev.x, prev.y, 3).stroke({ width: 1, color: COL_ROUTE, alpha: 0.7 });
        continue;
      }
      if (!dest) continue;
      // The server's fog-safe intended route replaces this legacy straight-line
      // fallback while delivery has not yet become observable.
      if (fleetsWithPendingIntent.has(shipId)) continue;
      const to = this.worldToScreen(dest);
      // Dashed line from the served sighting to its commanded destination.
      dashedLine(g, from.x, from.y, to.x, to.y, 6, 5);
      g.stroke({ width: 1, color: COL_ROUTE, alpha: 0.45 });
      g.circle(to.x, to.y, 3).stroke({ width: 1, color: COL_ROUTE, alpha: 0.7 });
    }

    // Pending fixed-destination orders: dashed/dim green, with the same Orders
    // selection emphasis as the outbound comet. Positions are served once at
    // issue; unlike the observed flight-plan line, this path is not consumed.
    for (const order of pendingIntents) {
      const path = order.intent_path!;
      const focusing = state.selectedOrderId !== null;
      const selectedOrder = state.selectedOrderId === order.id;
      const emphasis = focusing ? (selectedOrder ? 1 : 0.35) : 1;
      for (let i = 1; i < path.length; i++) {
        const from = this.worldToScreen(path[i - 1]);
        const to = this.worldToScreen(path[i]);
        dashedLine(g, from.x, from.y, to.x, to.y, 4, 5);
      }
      g.stroke({
        width: selectedOrder ? 1.8 : 1,
        color: COL_INTENT_ROUTE,
        alpha: (selectedOrder ? 0.58 : 0.24) * emphasis,
      });
    }

    // Prospective order — deliberately independent of the current route above,
    // so both remain on screen for comparison. Move previews wait for the
    // server's fog-safe lane plan; targeted verbs use their observed target
    // point directly. Every prospective leg uses a short 3/3 dash, visibly
    // distinct from the current route's longer 6/5 warp dashes.
    const intent = state.pendingIntent;
    if (!intent?.dest) return;
    const ghost = state.ghosts.find((x) => x.id === intent.shipId && x.own);
    if (!ghost) return;
    let prev = this.worldToScreen(ghost.pos);
    if (intent.verb === "move" && intent.path !== undefined) {
      if (intent.path.length > 0) {
        for (const leg of intent.path) {
          const p = this.worldToScreen(leg.pos);
          dashedLine(g, prev.x, prev.y, p.x, p.y, 3, 3);
          g.stroke({ width: leg.lane ? 1.7 : 1.25, color: COL_ROUTE_PREVIEW, alpha: leg.lane ? 0.9 : 0.72 });
          prev = p;
        }
      } else {
        const to = this.worldToScreen(intent.dest);
        dashedLine(g, prev.x, prev.y, to.x, to.y, 3, 3);
        g.stroke({ width: 1.25, color: COL_ROUTE_PREVIEW, alpha: 0.72 });
      }
    } else if (intent.verb !== "move") {
      const to = this.worldToScreen(intent.dest);
      dashedLine(g, prev.x, prev.y, to.x, to.y, 3, 3);
      g.stroke({ width: 1.35, color: COL_ROUTE_PREVIEW, alpha: 0.82 });
    }
    const ring = this.worldToScreen(intent.dest);
    dashedCircle(g, ring.x, ring.y, 5, 8);
    g.stroke({ width: 1.4, color: COL_ROUTE_PREVIEW, alpha: 0.95 });
  }

  /// Pool a celestial body sprite by id in the persistent bodyLayer (so we don't
  /// churn a Sprite per system per frame). Anchored centre; texture swapped if needed.
  private bodyFor(id: string, tex: Texture): Sprite {
    let sp = this.systemBodies.get(id);
    if (!sp) {
      sp = new Sprite(tex);
      sp.anchor.set(0.5);
      this.bodyLayer.addChild(sp);
      this.systemBodies.set(id, sp);
    } else if (sp.texture !== tex) {
      sp.texture = tex;
    }
    return sp;
  }

  /// The hub body: the WORMHOLE landmark sprite (swirling aperture + station)
  /// at the hub, over its teal glow (which stays in the background). Sized to
  /// out-scale every star on the map at every zoom: HUB_PX through mid zoom,
  /// then the late body-bloom curve grows it to its monumental native size at
  /// max — the top of the size hierarchy. The old mining-station sprite remains
  /// the fallback until the landmark art loads. Positioned each frame (zoom/pan).
  private drawHubBody(): void {
    const tex = this.texHub ?? this.texStation;
    if (!this.galaxy || !tex) return;
    if (!this.hubSprite) {
      this.hubSprite = new Sprite(tex);
      this.hubSprite.anchor.set(0.5);
      this.bodyLayer.addChild(this.hubSprite);
    }
    if (this.hubSprite.texture !== tex) this.hubSprite.texture = tex;
    const h = this.worldToScreen(this.galaxy.hub);
    this.hubSprite.position.set(h.x, h.y);
    this.hubSprite.scale.set(
      tex === this.texHub ? this.hubRenderedPx() / (HUB_ART_FILL * tex.width) : 28 / tex.width,
    );
  }

  /// The hub landmark's rendered VISIBLE size at the current zoom. Its late
  /// body band still targets the texture's NATIVE extent (fill × width = 0.93 ×
  /// 1254 ≈ 1166px visible), so the sprite-scale math below lands at exactly 1.0
  /// at max zoom — the hub is never upscaled. Before the landmark loads, stay at
  /// the marker size (the station fallback has no deep-zoom treatment anyway).
  private hubRenderedPx(): number {
    const maxPx = this.texHub ? HUB_ART_FILL * this.texHub.width : HUB_PX;
    return this.deepZoomPx(HUB_PX, maxPx, BODY_ZOOM_START, ZOOM_MAX_FACTOR);
  }

  /// Half the hub landmark's on-screen size — its click hit radius (main.ts) —
  /// capped so the max-zoom monument never swallows clicks meant for the fleets
  /// parked at the hub (ships are hit-tested first and stay under the cap).
  hubHitRadius(): number {
    return Math.min(this.hubRenderedPx() / 2, BODY_HIT_CAP_PX);
  }

  /// The ship art for a kind (null until loaded — primitive fallback covers it).
  private texFor(kind: ShipKind): Texture | null {
    switch (kind) {
      case "convoy": return this.texConvoy;
      case "raider": return this.texRaider;
      case "corvette": return this.texCorvette;
      case "colony": return this.texColony;
      case "scout": return this.texScout;
      // §ladder: the capital ladder (null until loaded — primitive fallback).
      case "destroyer": return this.texDestroyer;
      case "cruiser": return this.texCruiser;
      case "battleship": return this.texBattleship;
      case "dreadnought": return this.texDreadnought;
      case "titan": return this.texTitan;
      // §TCA: the Authority freighter reuses the bulk-hauler art; its neutral
      // TINT is what distinguishes it from a corporation's convoy.
      case "freighter": return this.texConvoy;
      // §ground: the troopship rides the colony hull until it has its own art.
      case "transport": return this.texColony;
      // §emplacements: the crane rides the bulk-hauler art until it has its own.
      case "builder": return this.texConvoy;
    }
  }

  /// The formation-art FAMILY for a flagship kind (colony has none — a colony
  /// fleet always draws the single colony ship + its count badge).
  private static fleetFamily(kind: ShipKind): FleetFamily | null {
    switch (kind) {
      case "convoy": return "freighter";
      case "raider": return "raider";
      case "corvette": return "corvette";
      case "scout": return "scout";
      case "colony": return null;
      // §ground: a landing force draws its single fat hull + count badge, like
      // the colony ship it shares art with.
      case "transport": return null;
      // §ladder: a capital IS the formation — always the single (placeholder)
      // hull + count badge, like the colony ship.
      case "destroyer":
      case "cruiser":
      case "battleship":
      case "dreadnought":
      case "titan":
        return null;
      case "freighter": return "freighter";
      // §emplacements: a builder fleet draws as the single working hull.
      case "builder": return null;
    }
  }

  /// The formation TIER from what the VIEWER knows about the fleet's size: the
  /// exact count when the composition is known (own fleet, or a rival inside
  /// sensor coverage), else the fog SIZE BUCKET. 1 → no formation (single ship),
  /// 2–3 → wing, 4–7 → squadron, 8+ → armada — the same breakpoints as the
  /// buckets, so the sprite never contradicts the count badge beside it.
  private static fleetTier(ghost: GhostView): FleetTier | null {
    const exact = fleetExactCount(ghost);
    if (exact !== null) {
      if (exact <= 1) return null;
      if (exact <= 3) return "wing";
      if (exact <= 7) return "squadron";
      return "armada";
    }
    switch (ghost.count_class) {
      case "one": return null;
      case "two_to_three": return "wing";
      case "four_to_seven": return "squadron";
      default: return "armada"; // 8–15, 16–30, 31+
    }
  }

  /// The marker art for a fleet: the formation sprite (family × tier) plus its
  /// canvas multiplier (TIER_SCALE × measured lead-ship calibration), or the
  /// single-ship sprite (mult 1) for fleets of one, colony fleets, and any
  /// formation art that failed to load. The multiplier applies to a target px
  /// computed against the SINGLE sprite's canvas, so the formation's LEAD ship
  /// renders at exactly the single sprite's size at every zoom — growing a
  /// fleet adds escorts around the flagship, it never inflates the flagship.
  /// §fleet-lod: the low-detail icon + calibration for a flagship kind, or null
  /// when the family has no icon (scout/colony) or it hasn't loaded yet. Only
  /// freighter (convoy), raider, and corvette carry icons.
  private lodIconMarker(kind: ShipKind): { tex: Texture; mult: number } | null {
    let tex: Texture | null = null;
    let fam: LodFamily | null = null;
    switch (kind) {
      case "convoy": tex = this.texIconFreighter; fam = "freighter"; break;
      case "raider": tex = this.texIconRaider; fam = "raider"; break;
      case "corvette": tex = this.texIconCorvette; fam = "corvette"; break;
      default: return null; // scout / colony — no icon
    }
    return tex ? { tex, mult: LOD_ICON_CALIB[fam] } : null;
  }

  private fleetMarker(ghost: GhostView): { tex: Texture; mult: number } | null {
    // §fleet-lod: far zoomed out, a single bold low-detail icon replaces the
    // detailed hull/formation (no fine detail is legible at that size anyway; the
    // count badge still conveys fleet size). Runs before the formation pick so it
    // covers single ships AND multi-ship fleets of these families.
    if (this.scale / this.fitScale() < LOD_ICON_ZOOM_MAX) {
      const icon = this.lodIconMarker(ghost.kind);
      if (icon) return icon;
    }
    const fam = Renderer.fleetFamily(ghost.kind);
    const tier = fam ? Renderer.fleetTier(ghost) : null;
    if (fam && tier) {
      const tex = this.texFleet.get(`${fam}_${tier}`);
      if (tex) return { tex, mult: TIER_SCALE[tier] * FLEET_LEAD_CALIB[fam][tier] };
    }
    const single = this.texFor(ghost.kind);
    return single ? { tex: single, mult: 1 } : null;
  }

  private ghostSprite(id: string): GhostSprite {
    let sp = this.ghosts.get(id);
    if (!sp) {
      const container = new Container();
      const cone = new Graphics();
      const ring = new Graphics();
      const body = new Graphics();
      const sprite = new Sprite(Texture.EMPTY);
      sprite.anchor.set(0.5);
      sprite.visible = false;
      const delayIcon = new Sprite(Texture.EMPTY);
      delayIcon.anchor.set(0.5);
      delayIcon.visible = false;
      delayIcon.eventMode = "static";
      delayIcon.cursor = "help";
      const delayTooltip = new Container();
      delayTooltip.eventMode = "none";
      delayTooltip.visible = false;
      const delayTipText = new Text({
        text: "Extremely Delayed Information",
        style: new TextStyle({
          fill: 0xffd56a,
          fontFamily: "ui-monospace, monospace",
          fontSize: 10,
          lineHeight: 13,
        }),
      });
      const delayTipPadX = 7;
      const delayTipPadY = 5;
      const delayTipW = delayTipText.width + delayTipPadX * 2;
      const delayTipH = delayTipText.height + delayTipPadY * 2;
      const delayTipBg = new Graphics()
        .roundRect(0, 0, delayTipW, delayTipH, 4)
        .fill({ color: 0x080b12, alpha: 0.96 })
        .stroke({ width: 1, color: COL_COMMS_DARK, alpha: 0.65 });
      delayTipText.position.set(delayTipPadX, delayTipPadY);
      delayTooltip.addChild(delayTipBg, delayTipText);
      delayIcon.on("pointerover", () => {
        if (delayIcon.visible) delayTooltip.visible = true;
      });
      delayIcon.on("pointerout", () => {
        delayTooltip.visible = false;
      });
      const label = new Text({ text: "", style: new TextStyle({ fill: COL_OTHER, fontFamily: "ui-monospace, monospace", fontSize: 9 }) });
      label.anchor.set(0, 0.5);
      const pip = new Graphics();
      // Fleet count badge: a small pill at the sprite's lower-right showing the
      // fleet size (exact when known, the fog bucket otherwise).
      const badge = new Graphics();
      const badgeText = new Text({ text: "", style: new TextStyle({ fill: 0xffffff, fontFamily: "ui-monospace, monospace", fontSize: 9, fontWeight: "bold" }) });
      badgeText.anchor.set(0.5, 0.5);
      // Pip is topmost so the friend/foe tag is never hidden by the sprite/label.
      container.addChild(cone, ring, body, sprite, label, badge, badgeText, pip, delayIcon, delayTooltip);
      this.ghostsLayer.addChild(container);
      sp = { container, cone, body, sprite, delayIcon, delayTooltip, label, ring, pip, badge, badgeText, seen: true };
      this.ghosts.set(id, sp);
    }
    return sp;
  }

  private servedStreamPinned(ghost: GhostView, simTime: number): boolean {
    const previous = this.servedGhostFrames.get(ghost.id);
    if (previous?.simTime === simTime) return previous.pinned;
    // A healthy LIVE replay advances on every View. Exact repetition means the
    // server deliberately pinned this stream, so extrapolating its non-zero vel
    // would create a tug away from the served point until the next View reset it.
    const pinned = previous !== undefined
      && previous.pos.x === ghost.pos.x
      && previous.pos.y === ghost.pos.y;
    this.servedGhostFrames.set(ghost.id, { pos: { ...ghost.pos }, simTime, pinned });
    return pinned;
  }

  private spawnReacquisition(oldPos: Vec2, newPos: Vec2): void {
    const root = new Container();
    const pulse = new Graphics();
    root.addChild(pulse);
    this.reacquireLayer.addChild(root);
    this.reacquireFx.push({
      root,
      pulse,
      oldPos: { ...oldPos },
      newPos: { ...newPos },
      startedMs: performance.now(),
    });
  }

  private updateReacquisitions(): void {
    const now = performance.now();
    for (let i = this.reacquireFx.length - 1; i >= 0; i--) {
      const fx = this.reacquireFx[i];
      const t = clamp01((now - fx.startedMs) / REENTRY_FX_MS);
      if (t >= 1) {
        this.reacquireLayer.removeChild(fx.root);
        fx.root.destroy({ children: true });
        this.reacquireFx.splice(i, 1);
        continue;
      }
      const oldScreen = this.worldToScreen(fx.oldPos);
      const newScreen = this.worldToScreen(fx.newPos);
      const eased = t * t * (3 - 2 * t);
      const head = {
        x: oldScreen.x + (newScreen.x - oldScreen.x) * eased,
        y: oldScreen.y + (newScreen.y - oldScreen.y) * eased,
      };
      fx.pulse.clear();
      fx.pulse.moveTo(oldScreen.x, oldScreen.y).lineTo(head.x, head.y).stroke({
        width: 2.2,
        color: COL_OWN,
        alpha: 0.45 * (1 - t),
      });
      fx.pulse.circle(head.x, head.y, 4 + 10 * t).stroke({
        width: 1.2,
        color: COL_OWN,
        alpha: 0.55 * (1 - t),
      });
    }
  }

  /// §zoom-continuum: the smoothstep shared by two distinct size bands.
  /// Machines use the default 12→24 interval and freeze thereafter; bodies pass
  /// 72→96 explicitly. Zero-slope endpoints make either interval seamless.
  private deepZoomPx(
    basePx: number,
    maxPx: number,
    startR = SHIP_NATIVE_ZOOM_START,
    endR = MACHINE_ZOOM_END,
  ): number {
    const r = this.scale / this.fitScale();
    if (r <= startR) return basePx;
    const t = Math.min((r - startR) / (endR - startR), 1);
    const s = t * t * (3 - 2 * t); // smoothstep — gentle growth, not linear
    return basePx + (maxPx - basePx) * s;
  }

  /// On-screen ship size (px) as a function of the current zoom, in TWO phases:
  ///  1. Normal / indicator: base × clamp(r, SHIP_ZOOM_MIN, SHIP_ZOOM_MAX) — the
  ///     small map markers, unchanged, across the whole normal zoom range.
  ///  2. Neighborhood: r=12→24 ramps the indicator to SHIP_MAX_PX, then it
  ///     freezes throughout the approach so increasing world-space separation
  ///     remains visible beside the later body bloom.
  /// All kinds converge to the SAME max size: up close the art's SHAPE
  /// distinguishes convoy vs raider, so identical max size is intended.
  private shipSizePx(kind: ShipKind): number {
    // §ladder: capitals scale by MASS CLASS — each rung visibly bigger, the
    // Titan the largest thing flying (procedural placeholder sizing).
    const capital: Partial<Record<ShipKind, number>> = { destroyer: 52, cruiser: 60, battleship: 70, dreadnought: 82, titan: 96 };
    const base = capital[kind]
      ?? (kind === "convoy" ? SHIP_PX_CONVOY : kind === "raider" ? SHIP_PX_RAIDER : kind === "corvette" ? SHIP_PX_CORVETTE : kind === "colony" ? SHIP_PX_COLONY : SHIP_PX_SCOUT);
    const r = this.scale / this.fitScale();
    const indicator = base * Math.max(SHIP_ZOOM_MIN, Math.min(SHIP_ZOOM_MAX, r));
    return this.deepZoomPx(indicator, SHIP_MAX_PX);
  }

  /// Half the ship's CURRENT on-screen size — the click hit radius, so ships stay
  /// clickable as they enlarge in the 12→24 machine band (capped under the body
  /// hit cap, so ships always win the first-pass hit-test over grown bodies).
  /// Consumed by main.ts's map hit-test.
  shipHitRadius(kind: ShipKind): number {
    return this.shipSizePx(kind) / 2;
  }

  /// On-screen size for a completed open-space structure. It follows the same
  /// zoom language as ships: compact map marker, a 12→24 neighborhood ramp,
  /// then a fixed 96px machine throughout the extended approach.
  private emplacementSizePx(): number {
    const r = this.scale / this.fitScale();
    const indicator = EMPLACEMENT_PX * Math.max(SHIP_ZOOM_MIN, Math.min(SHIP_ZOOM_MAX, r));
    return this.deepZoomPx(indicator, EMPLACEMENT_MAX_PX);
  }

  /// Keep normal-zoom emplacement clicking unchanged (18px), while allowing the
  /// hit target to follow the sprite as it grows at deep zoom.
  emplacementHitRadius(): number {
    return Math.max(18, this.emplacementSizePx() / 2);
  }

  /// Half the fleet MARKER's current on-screen size — like shipHitRadius, but
  /// including the formation sprite's canvas multiplier, so a squadron's click
  /// target (and the overlays anchored to it) covers the whole formation, not
  /// just the lead ship. Consumed by main.ts's map hit-test and by drawGhost's
  /// pip/badge anchors.
  fleetHitRadius(ghost: GhostView): number {
    const marker = this.fleetMarker(ghost);
    return this.shipHitRadius(ghost.kind) * (marker ? marker.mult : 1);
  }

  /// §emplacements: MIRRORS `emplace::site_check` IN THE SIM.
  ///
  /// The map must preview exactly the rule the server will enforce, or a site
  /// can look legal and then be silently refused — the one failure mode a
  /// placement UI must not have. Kept next to the drawing so the two cannot
  /// drift without someone noticing.
  siteError(kind: string, p: { x: number; y: number }, state: ViewState): string | null {
    if (kind === "hyperspace_buoy" || kind === "hyperspace_repeater" || kind === "hyperspace_sensor") {
      const lanes = state.galaxy?.lanes ?? [];
      const onALane = lanes.some((l) => {
        // Distance to the CENTERLINE, not to the nearest point — same fix the
        // sim needed, for the same reason: samples are far further apart than a
        // ribbon is wide, so a point-wise test rejects points lying on the line.
        for (let i = 1; i < l.points.length; i++) {
          const a = l.points[i - 1];
          const b = l.points[i];
          const abx = b.x - a.x;
          const aby = b.y - a.y;
          const len2 = abx * abx + aby * aby;
          const t = len2 > 1e-9 ? Math.max(0, Math.min(1, ((p.x - a.x) * abx + (p.y - a.y) * aby) / len2)) : 0;
          const dx = p.x - (a.x + abx * t);
          const dy = p.y - (a.y + aby * t);
          if (Math.hypot(dx, dy) <= l.half_width) return true;
        }
        return false;
      });
      if (!onALane)
        return kind === "hyperspace_buoy"
          ? "A hyperspace buoy has to sit in a lane — it is a long-throw comm relay."
          : kind === "hyperspace_repeater"
            ? "A hyperspace repeater has to sit in a lane — it is a short-throw comm relay."
            : "A hyperspace sensor has to sit in a lane — it listens to one.";
    }
    const tooClose = (state.emplacements ?? []).some(
      (e) => Math.hypot(e.pos.x - p.x, e.pos.y - p.y) < EMPLACEMENT_MIN_SPACING,
    );
    return tooClose ? "Too close to one of your own." : null;
  }

  /// §emplacements: draw what is standing out there. (The build verb lives on
  /// the Construction Ship's panel and builds where the ship is parked — there
  /// is no cursor siting preview any more.)
  private drawEmplacements(state: ViewState): void {
    const g = this.emplaceGfx;
    const selection = this.emplaceSelectionGfx;
    g.clear();
    selection.clear();
    const seenSprites = new Set<string>();
    for (const e of state.emplacements ?? []) {
      const p = this.worldToScreen(e.pos);
      const buoy = e.kind === "hyperspace_buoy";
      const repeater = e.kind === "hyperspace_repeater";
      const listener = e.kind === "hyperspace_sensor";
      // §emplacements: a RIVAL's structure wears the threat colour, like their
      // ghosts — the shape still says which kind it is, the colour says whose.
      // (A rival's is only ever listed inside your sensor coverage.)
      const col = e.own === false
        ? COL_THREAT
        : buoy ? 0x6fd0ff : repeater ? 0x8fe3a0 : listener ? 0xd9a8ff : 0x8fe3a0;
      // A sensor's coverage is the reason it exists, so it is drawn.
      if (e.sensor_range > 0) {
        g.circle(p.x, p.y, e.sensor_range * this.scale).stroke({ width: 1, color: col, alpha: 0.18 });
      }
      const tex = buoy
        ? this.texHyperspaceBuoy
        : repeater ? this.texHyperspaceRepeater
        : e.kind === "deep_space_sensor" ? this.texDeepSpaceSensor : null;
      if (tex) {
        let sp = this.emplacementSprites.get(e.id);
        if (!sp) {
          sp = new Sprite(tex);
          sp.anchor.set(0.5);
          this.emplacementSpriteLayer.addChild(sp);
          this.emplacementSprites.set(e.id, sp);
        } else if (sp.texture !== tex) {
          sp.texture = tex;
        }
        sp.position.set(p.x, p.y);
        const size = this.emplacementSizePx() * (repeater ? 0.78 : 1);
        sp.scale.set(size / tex.width);
        // Preserve the established map grammar: own structures use their
        // natural cyan/green art; a revealed rival structure is threat-red.
        sp.tint = e.own === false ? COL_THREAT : 0xffffff;
        sp.alpha = e.own === false ? 0.94 : 1;
        sp.visible = true;
        seenSprites.add(e.id);
      } else if (buoy || repeater) {
        // Both are full relays; the smaller center pip distinguishes the cheap,
        // short-throw repeater size when art has not loaded.
        g.circle(p.x, p.y, 5).stroke({ width: 1.5, color: col, alpha: 0.9 });
        g.circle(p.x, p.y, repeater ? 1 : 1.5).fill({ color: col, alpha: 0.9 });
      } else if (listener) {
        // A tripwire reads as a bracket ACROSS the lane — a thing you pass
        // through, not a node you talk to.
        g.moveTo(p.x - 5, p.y - 5).lineTo(p.x - 5, p.y + 5);
        g.moveTo(p.x + 5, p.y - 5).lineTo(p.x + 5, p.y + 5);
        g.stroke({ width: 1.5, color: col, alpha: 0.9 });
        g.circle(p.x, p.y, 1.2).fill({ color: col, alpha: 0.9 });
      } else {
        // A sensor reads as a dish: a wedge, so the two never look alike.
        g.moveTo(p.x - 5, p.y + 4).lineTo(p.x, p.y - 5).lineTo(p.x + 5, p.y + 4).closePath();
        g.stroke({ width: 1.5, color: col, alpha: 0.9 });
      }
      // Selection ring — the same affordance a selected fleet gets, so a
      // structure reads as a clickable object rather than scenery.
      if (state.selectedEmplacementId === e.id) {
        // §comms-infra: selected communications sites disclose their nominal
        // throw in WORLD su. The actual usable wire still follows lane arcs and
        // needs overlapping sites; this ring makes the balance radius legible
        // without pretending every standing relay is a sensor.
        if ((buoy || repeater) && e.relay_throw > 0) {
          const commRadius = e.relay_throw * this.scale;
          const dashPairs = Math.max(12, Math.min(360, Math.ceil(Math.PI * 2 * commRadius / 12)));
          dashedCircle(selection, p.x, p.y, commRadius, dashPairs * 2);
          selection.stroke({ width: 1.4, color: col, alpha: 0.72 });
        }
        const radius = tex ? this.emplacementSizePx() * (repeater ? 0.78 : 1) / 2 + 4 : 12;
        selection.circle(p.x, p.y, radius).stroke({ width: 1.5, color: col, alpha: 0.95 });
      }
    }
    // A completed demolition or a newly fog-hidden rival stops arriving in the
    // wire view. Retire its pooled sprite on that same view update.
    for (const [id, sp] of this.emplacementSprites) {
      if (seenSprites.has(id)) continue;
      this.emplacementSpriteLayer.removeChild(sp);
      sp.destroy();
      this.emplacementSprites.delete(id);
    }
  }

  private drawGhost(ghost: GhostView, state: ViewState, dt: number): { x: number; y: number } {
    const sp = this.ghostSprite(ghost.id);
    sp.seen = true;
    const own = ghost.own;
    const isDark = own && !ghost.in_comms;
    const inTunnel = ghostInTunnel(ghost);
    const showDarkArrow = isDark;
    const liveSim = liveSimTime();
    const servedPinned = own && this.servedStreamPinned(ghost, state.simTime);
    const tunnelBookmark = state.tunnelBookmarks[ghost.id];
    // LIVE advances the delayed replay through the fraction of the current View
    // gap unless the served stream itself is pinned. Realspace DARK uses each
    // arrived report; TUNNEL is fixed to the first served point beyond the edge.
    const target = inTunnel
      ? tunnelBookmark?.pos ?? ghost.pos
      : isDark || servedPinned
      ? { x: ghost.pos.x, y: ghost.pos.y }
      : { x: ghost.pos.x + ghost.vel.x * dt, y: ghost.pos.y + ghost.vel.y * dt };
    const prev = sp.shown;
    const nowMs = performance.now();
    const enteredTunnel = own && sp.inTunnel === false && inTunnel;
    const exitedTunnel = own && sp.inTunnel === true && !inTunnel && !!prev;
    const enteredDark = own && sp.inComms === true && !ghost.in_comms;
    const reentered = own && sp.inComms === false && ghost.in_comms && !!prev && !exitedTunnel;
    if (enteredTunnel) {
      // The exact served bubble-crossing frame becomes a stationary bookmark.
      // Use the established sprite→arrow morph without ever advancing it again.
      sp.darkMorphMs = nowMs;
      sp.reentry = undefined;
    }
    if (enteredDark) sp.darkMorphMs = nowMs;
    if (reentered && prev) {
      sp.reentry = { from: { ...prev }, to: { ...target }, startedMs: nowMs };
      this.spawnReacquisition(prev, target);
    }
    if (exitedTunnel) {
      // This point exists only because its served drive/bubble fact arrived.
      // Reuse the reacquisition pulse at one stationary, fog-safe position.
      this.spawnReacquisition(target, target);
    }

    let reentryT = 1;
    if (exitedTunnel) {
      sp.shown = { ...target };
    } else if (sp.reentry) {
      reentryT = clamp01((nowMs - sp.reentry.startedMs) / REENTRY_FX_MS);
      const eased = reentryT * reentryT * (3 - 2 * reentryT);
      sp.shown = {
        x: sp.reentry.from.x + (sp.reentry.to.x - sp.reentry.from.x) * eased,
        y: sp.reentry.from.y + (sp.reentry.to.y - sp.reentry.from.y) * eased,
      };
      if (reentryT >= 1) sp.reentry = undefined;
    } else if (!prev || isDark) {
      sp.shown = { ...target };
    } else {
      const jump = Math.hypot(target.x - prev.x, target.y - prev.y);
      const snapAt = Math.max(SMOOTH_SNAP_SU, Math.hypot(ghost.vel.x, ghost.vel.y) * SMOOTH_SNAP_S);
      if (!own && jump > snapAt) {
        sp.shown = { ...target };
      } else {
        const k = 1 - Math.exp(-SMOOTH_RATE * Math.max(this.frameDt, 1 / 240));
        sp.shown = { x: prev.x + (target.x - prev.x) * k, y: prev.y + (target.y - prev.y) * k };
      }
    }
    const s = this.worldToScreen(sp.shown);
    sp.container.position.set(s.x, s.y);
    sp.inComms = own ? ghost.in_comms : undefined;
    sp.inTunnel = own ? inTunnel : undefined;

    const headingVel = inTunnel ? tunnelBookmark?.vel ?? ghost.vel : ghost.vel;
    const angle = Math.atan2(headingVel.y, headingVel.x);
    let arrowMix = showDarkArrow ? 1 : sp.reentry ? 1 - reentryT : 0;
    if (showDarkArrow && sp.darkMorphMs !== undefined) {
      arrowMix = clamp01((nowMs - sp.darkMorphMs) / 220);
      if (arrowMix >= 1) sp.darkMorphMs = undefined;
    }

    // §fog: NO UNCERTAINTY CIRCLE. There used to be one here — radius
    // `age x hull speed` — drawn when you selected a contact. It was deleted
    // rather than fixed: the hull speed is the THRUSTER speed, so a lane rider's
    // reach was understated fifty-fold, and no circle can be honest where speed
    // depends on standing on a road. The panel's SEEN age and the sighting's
    // drive state carry the same information without the false precision, and
    // the amber intercept estimate answers the question the circle was reached
    // for. `sp.cone` still carries the survey ring, pending badge, threat ring
    // and signature flare below.
    sp.cone.clear();
    // §explore Part 2: SURVEY PROGRESS RING — owner-only (the field is only ever
    // sent for own fleets): an arc filling clockwise as the dwell runs, in the
    // scout's own cyan. A rival sees none of this — only the louder signature.
    if (own && !showDarkArrow && ghost.survey_progress != null) {
      const p = Math.max(0, Math.min(1, ghost.survey_progress));
      const r = 9;
      sp.cone.circle(0, 0, r).stroke({ width: 1, color: COL_OWN, alpha: 0.25 });
      if (p > 0.01) {
        sp.cone.arc(0, 0, r, -Math.PI / 2, -Math.PI / 2 + p * Math.PI * 2).stroke({ width: 2, color: COL_OWN, alpha: 0.9 });
      }
    }
    // The delay cue is UI-sized, not world-sized: map zoom moves the sighting
    // but never changes the icon's 36px screen footprint. On a tunnel bookmark
    // it is deliberately dimmed with the arrow: this is lost live tracking,
    // not the later arrived-light crawl of a realspace-dark fleet.
    sp.delayIcon.visible = showDarkArrow && this.texCommsDelay !== null;
    if (!sp.delayIcon.visible) sp.delayTooltip.visible = false;
    if (sp.delayIcon.visible && this.texCommsDelay) {
      if (sp.delayIcon.texture !== this.texCommsDelay) sp.delayIcon.texture = this.texCommsDelay;
      sp.delayIcon.scale.set(DARK_DELAY_ICON_PX / this.texCommsDelay.width);
      sp.delayIcon.position.set(-22, 18);
      sp.delayIcon.alpha = (inTunnel ? 0.58 : 0.95) * arrowMix;
      const tipW = sp.delayTooltip.width;
      const tipH = sp.delayTooltip.height;
      const desiredLeft = s.x - 22 - tipW / 2;
      const clampedLeft = Math.max(6, Math.min(this.viewW - tipW - 6, desiredLeft));
      const above = -tipH - 8;
      const tipY = s.y + above >= 6 ? above : 42;
      sp.delayTooltip.position.set(clampedLeft - s.x, tipY);
    }
    // §order-lifecycle: is this own fleet's relevant order still unconfirmed (its
    // compliance light hasn't returned)? An estimate expiring cannot resolve this
    // state: only the server's arrived-evidence event retires the lifecycle row.
    // While so, the commanded-heading hint is
    // drawn DASHED (= commanded/claimed) and a pending badge shows; both resolve
    // to the normal SOLID hint / no badge at echo (= observed). The TWO pending
    // phases get subtly different treatments, mirroring the fleet panel's ◈/◔
    // vocabulary with the SAME estimated boundary (liveSim vs arrives_at, then
    // arrives_at):
    //   phase 1 IN TRANSIT (before arrives_at): the fleet doesn't know yet —
    //     hollow-diamond badge (the signal motif), sparser/dimmer dashes.
    //   phase 2 PRESUMED DELIVERED: the forecast says they have it and are executing,
    //     you just haven't seen it — quarter-filled clock, tighter/brighter dashes.
    // The 1.5s suppression matches the panel's LIFECYCLE_MIN_S (no sub-second
    // flicker for a fleet at the command center).
    const queue = own ? state.pendingOrders.get(ghost.id) ?? [] : [];
    const activeQueue = queue.filter((order) => !order.lost);
    const selectedPend = state.selectedOrderId === null
      ? undefined
      : activeQueue.find((p) => p.id === state.selectedOrderId);
    const pend = selectedPend ?? activeQueue[activeQueue.length - 1];
    const unconfirmed = !!pend && pend.response_at - pend.arrives_at >= 1.5;
    const inTransit = unconfirmed && liveSim < pend!.arrives_at; // phase 1, else phase 2
    const selectedDelivered = !!selectedPend && liveSim >= selectedPend.arrives_at;

    // (No "probably advanced to about here" pip: its length was the uncertainty
    // radius, and `drawOrders` already draws the fleet's WHOLE commanded route —
    // the real flight plan, lane legs and all. The pending badge below still
    // carries the order phase.)

    // Pending badge, own-cyan, just off the pip while the order is unconfirmed —
    // a subtle state tag, not an alarm. Gone at echo. The glyph steps with the
    // phase at arrives_at, mirroring the panel: ◈ hollow diamond while the
    // signal is IN TRANSIT, ◔ quarter-filled clock while a response is expected.
    if (unconfirmed) {
      const bx = 11;
      const by = -(this.fleetHitRadius(ghost) + 5);
      if (selectedDelivered) {
        const pulse = 0.5 + 0.5 * Math.sin(performance.now() / 260);
        sp.cone.circle(bx, by, 6.2 + pulse * 1.8).stroke({
          width: 1.8,
          color: COL_COMMAND,
          alpha: 0.7 + pulse * 0.25,
        });
      }
      if (inTransit) {
        // ◈ — hollow diamond (the signal motif) with a tiny center pip.
        const dr = 3.8;
        sp.cone.poly([bx, by - dr, bx + dr, by, bx, by + dr, bx - dr, by]).stroke({ width: 1.2, color: COL_OWN, alpha: 0.85 });
        sp.cone.circle(bx, by, 0.9).fill({ color: COL_OWN, alpha: 0.85 });
      } else {
        // ◔ — clock outline with the first quarter filled (delivered, unechoed).
        sp.cone.circle(bx, by, 3.6).stroke({ width: selectedDelivered ? 1.8 : 1.2, color: COL_OWN, alpha: selectedDelivered ? 1 : 0.85 });
        sp.cone.moveTo(bx, by).arc(bx, by, 3.6, -Math.PI / 2, 0).lineTo(bx, by).fill({ color: COL_OWN, alpha: selectedDelivered ? 1 : 0.85 });
      }
    }
    // Detected rival raider = a threat contact (it's otherwise invisible). Make
    // it unmistakable with a pulsing alert ring — this is your only warning.
    if (!own && ghost.kind === "raider") {
      const pulse = 0.5 + 0.5 * Math.sin(performance.now() / 230);
      sp.cone.circle(0, 0, 13 + pulse * 7).stroke({ width: 1.6, color: COL_THREAT, alpha: 0.35 + 0.45 * pulse });
    }

    // §Part 4 SIGNATURE FLARE: a LOUD dark contact (big and/or at flank speed,
    // signature > 1) gets a steady plume/halo — distinct from the pulsing threat
    // ring — that grows with how loud it is. "Flank speed lights you up."
    if (!own && ghost.signature != null && ghost.signature > 1.05) {
      const loud = Math.min((ghost.signature - 1) / 1.5, 1); // 0..1 over 1..2.5
      const r = 16 + loud * 16;
      sp.cone.circle(0, 0, r).fill({ color: COL_THREAT, alpha: 0.04 + 0.06 * loud });
      sp.cone.circle(0, 0, r).stroke({ width: 1, color: COL_THREAT, alpha: 0.2 + 0.3 * loud });
    }

    // Selection ring.
    sp.ring.clear();
    if (state.selectedShipId === ghost.id) {
      sp.ring.circle(0, 0, 13).stroke({ width: 1.5, color: 0xffffff, alpha: 0.8 });
    }

    // The ship BODY: a top-down sprite rotated to heading, sized by kind (convoy
    // reads LARGER than the nimble raider — the asymmetry at a glance), rendered in
    // its NATURAL art with NO per-syndicate tint (own/rival are distinguished by
    // other cues — label, threat ring, selection — with a dedicated ownership
    // indicator still TBD). Faded by staleness: fade applies to own ships too, so a
    // distant (stale) own ship visibly dims while one near the command center stays
    // crisp — with a higher floor so you never "lose" your fleet.
    const displayAge = inTunnel && tunnelBookmark
      ? Math.max(0, liveSim - tunnelBookmark.reportedAt)
      : ghost.age;
    const fade = Math.min(displayAge / FADE_AGE_S, 1);
    const observedAlpha = own ? Math.max(0.62, 0.97 - 0.4 * fade) : Math.max(0.4, 0.95 - 0.55 * fade);
    const alpha = observedAlpha * (inTunnel ? 0.38 : showDarkArrow ? 0.6 : 1);
    // Marker art: the single-ship sprite, or a FORMATION sprite when the viewer
    // knows this fleet is 2+ (family × tier — see fleetMarker). The target px is
    // computed against the SINGLE sprite's canvas and then multiplied by the
    // formation's calibrated factor, so the LEAD ship holds exactly the single
    // sprite's size across tier changes — a growing fleet gains escorts, with
    // no flagship size pop (e.g. crossing 3 → 4 ships).
    const marker = this.fleetMarker(ghost);
    sp.body.clear();
    sp.body.scale.set(1);
    if (arrowMix > 0) {
      // DARK is one heading arrow, never speculative hull art. Realspace dark
      // advances only on arrived reports; the amber tunnel variant is a fixed,
      // dim bookmark at entry. Both dissolve against the live sprite in place.
      sp.body.scale.set(DARK_ARROW_SCALE);
      sp.sprite.visible = !!marker && arrowMix < 1;
      if (marker && sp.sprite.visible) {
        if (sp.sprite.texture !== marker.tex) sp.sprite.texture = marker.tex;
        const targetPx = this.shipSizePx(ghost.kind) * marker.mult;
        sp.sprite.scale.set(targetPx / marker.tex.width);
        sp.sprite.rotation = angle + SHIP_ART_FACING;
        sp.sprite.tint = 0xffffff;
        sp.sprite.alpha = alpha * (1 - arrowMix);
      }
      const pulse = inTunnel ? 0.42 : 0.75 + 0.25 * Math.sin(performance.now() / 220);
      const arrowColor = inTunnel ? COL_COMMS_DARK : COL_OWN;
      sp.body
        .moveTo(6, 0)
        .lineTo(-4, -4.5)
        .moveTo(6, 0)
        .lineTo(-4, 4.5)
        .stroke({ width: 1.8, color: arrowColor, alpha: pulse * arrowMix });
      sp.body.circle(-1.5, 0, 1.2).fill({ color: arrowColor, alpha: pulse * arrowMix });
      sp.body.rotation = angle;
    } else if (marker) {
      sp.sprite.visible = true;
      if (sp.sprite.texture !== marker.tex) sp.sprite.texture = marker.tex;
      // Size vs zoom: a small indicator through normal zoom, ramping to
      // SHIP_MAX_PX in the deepest band (see shipSizePx / the size hierarchy).
      // Always ≤ the art's native px, so sprites stay downscale-crisp.
      const targetPx = this.shipSizePx(ghost.kind) * marker.mult;
      sp.sprite.scale.set(targetPx / marker.tex.width);
      sp.sprite.rotation = angle + SHIP_ART_FACING;
      sp.sprite.tint = 0xffffff; // natural art — no per-syndicate tint
      sp.sprite.alpha = alpha;
    } else {
      // Primitive triangle fallback until the art loads (syndicate-neutral).
      sp.sprite.visible = false;
      const len = ghost.kind === "convoy" ? 9 : 7;
      const wid = ghost.kind === "convoy" ? 6 : 3.5;
      sp.body.poly([len, 0, -len * 0.7, -wid, -len * 0.7, wid]).fill({ color: COL_SHIP_NEUTRAL, alpha });
      if (ghost.kind === "convoy") sp.body.circle(0, 0, 1.6).fill({ color: 0x05070d, alpha: 0.8 });
      sp.body.rotation = angle;
    }

    // Ownership PIP — a small, always-on friend/foe tag riding just above the ship:
    // a cyan diamond = YOURS (COL_OWN), red = RIVAL (COL_OTHER). Now that the hull
    // carries no ownership tint, THIS is the primary own-vs-rival cue. Drawn in
    // SCREEN space (a child at a fixed LOCAL offset, so it never rotates with heading
    // and keeps a consistent screen size), sat just above the sprite's current
    // on-screen extent, and sized in screen px (gently clamped so it neither balloons
    // nor vanishes across zoom). It keeps a HIGH alpha floor so ownership stays
    // readable even on a stale/faded ship — the pip is exactly the cue that must
    // SURVIVE the staleness fade (unlike the old tint, which washed out). A dark rim
    // keeps it legible over bright cues (sensor teal, threat rings). The diamond
    // shape reads distinctly from the many circular cues (cones/rings/sensor). This
    // is ADDITIVE — it doesn't touch the cone, threat ring, selection ring, or label.
    // (Ownership is BINARY here; a future enhancement could key the pip color per
    // rival syndicate by owner id, with your ships fixed cyan.)
    const pip = sp.pip;
    pip.clear();
    // §syndicates: own = cyan, ALLY (light-delayed known member) = green, rival = red.
    const pipCol = own ? COL_OWN : ghost.tca ? COL_TCA : ghost.pirate ? COL_PIRATE : ghost.ally ? COL_ALLY : COL_OTHER;
    const half = this.fleetHitRadius(ghost); // half the MARKER's current on-screen size (formation included)
    const pipR = Math.max(3.2, Math.min(8, half * 0.14));
    const pipY = -(half + pipR + 5); // just above the sprite's top edge, at every zoom
    const pipA = Math.max(0.85, 0.97 - 0.25 * fade); // high floor — survives staleness
    const diamond = (cy: number, rr: number): number[] => [0, cy - rr, rr, cy, 0, cy + rr, -rr, cy];
    if (arrowMix < 0.5) {
      pip.poly(diamond(pipY, pipR + 1.3)).fill({ color: 0x05070d, alpha: 0.7 * pipA }); // dark rim for contrast
      pip.poly(diamond(pipY, pipR)).fill({ color: pipCol, alpha: pipA });
    }

    // Label: threat warning for raiders, cargo manifest for convoys (shown only
    // when known — i.e. within sensor range), staleness everywhere it matters.
    const sel = state.selectedShipId === ghost.id;
    // Honest staleness, shown finer-grained when fresh (near the command center).
    const stale = `Δ${displayAge.toFixed(displayAge < 10 ? 1 : 0)}s`;
    let txt = "";
    let col = COL_OTHER;
    let lalpha = 0.85;
    if (inTunnel) {
      txt = `HYPERSPACE ENTRY  ${stale}`;
      col = COL_COMMS_DARK;
      lalpha = 0.62;
    } else if (showDarkArrow) {
      txt = `${ghost.kind.replaceAll("_", " ").toUpperCase()} SIGNAL  ${stale}`;
      col = COL_COMMS_DARK;
      lalpha = 0.82;
    } else if (ghost.kind === "raider" && !own) {
      txt = `⚠ RAIDER  ${stale}`;
      col = COL_THREAT;
      lalpha = 0.95;
    } else if (ghost.kind === "corvette" && !own) {
      // A rival corvette BROADCASTS (a declared escort deters): a visible
      // defender, not an attack alarm.
      txt = `ESCORT  ${stale}`;
      col = COL_OTHER;
      lalpha = 0.85;
    } else if (ghost.kind === "colony" && !own) {
      // A rival COLONY SHIP broadcasting its voyage: someone's expansion,
      // telegraphed — the loudest strategic signal on the map.
      txt = `COLONY SHIP  ${stale}`;
      col = COL_REPORT; // gold — this is intel worth acting on
      lalpha = 0.95;
    } else if (ghost.kind === "scout" && !own) {
      // A detected rival scout: a contact worth knowing about (someone is
      // LOOKING at you), but not an attack alarm — no pulsing threat ring.
      txt = `SCOUT  ${stale}`;
      col = COL_OTHER;
      lalpha = 0.9;
    } else if (own) {
      // Own ships are light-delayed too now — always surface staleness so the fog
      // reads as "reporting from Xs ago," not a glitch. Convoys also show cargo.
      const cargo = ghost.kind === "convoy"
        ? (ghost.cargo ? `${label(ghost.cargo.commodity)} ×${ghost.cargo.units}  ` : "")
        : "";
      const response = showDarkArrow && pend && !pend.lost
        ? pend.response_on_reentry
          ? "  response unknown"
          : pend.response_at > liveSim
            ? `  response ~${Math.ceil(pend.response_at - liveSim)}s`
            : `  response overdue ~${Math.ceil(liveSim - pend.response_at)}s`
        : "";
      txt = `${cargo}${stale}${response}`;
      col = COL_OWN;
      lalpha = sel ? 0.95 : 0.7;
    } else if (ghost.kind === "convoy") {
      const cargo = ghost.cargo ? `${label(ghost.cargo.commodity)} ×${ghost.cargo.units}` : "cargo ?";
      txt = `${cargo}  ${stale}`;
      col = ghost.cargo ? COL_REPORT : COL_OTHER; // known cargo = gold (intel!)
      lalpha = 0.9;
    } else if (ghost.kind === "freighter") {
      // §TCA: an Authority hull is named, in the Authority's own steel-blue —
      // neutral against both the own and rival palettes. Without this branch it
      // fell through every case and drew NO label at all.
      txt = `AUTHORITY  ${stale}`;
      col = COL_TCA;
      lalpha = 0.9;
    }
    sp.label.text = txt;
    sp.label.style.fill = col;
    sp.label.alpha = lalpha * (inTunnel ? 0.65 : showDarkArrow ? 0.6 : 1);
    sp.label.position.set(11, -10);

    // FLEET COUNT BADGE (§13.1 intel ladder). Exact Σ when the composition is
    // known (your own fleet, or a rival inside your sensor coverage); otherwise
    // the fog SIZE BUCKET ("4–7"), drawn dimmer to read as an estimate. A
    // fleet-of-one shows no badge — it looks exactly like the old single ship.
    const exact = fleetExactCount(ghost);
    let badgeStr = "";
    let estimate = false;
    if (exact !== null) {
      if (exact > 1) badgeStr = String(exact);
    } else if (ghost.count_class !== "one") {
      badgeStr = countClassLabel(ghost.count_class);
      estimate = true;
    }
    sp.badge.clear();
    if (badgeStr && arrowMix < 0.5) {
      const halfB = this.fleetHitRadius(ghost);
      const w = Math.max(13, badgeStr.length * 6 + 7);
      const h = 12;
      const bx = halfB * 0.66;
      const by = halfB * 0.55;
      const edge = own ? COL_OWN : ghost.tca ? COL_TCA : ghost.pirate ? COL_PIRATE : ghost.ally ? COL_ALLY : COL_OTHER;
      const bAlpha = Math.max(0.85, 0.97 - 0.25 * fade) * (showDarkArrow ? 0.6 : 1);
      sp.badge
        .roundRect(bx - w / 2, by - h / 2, w, h, 5)
        .fill({ color: 0x05070d, alpha: 0.82 * bAlpha })
        .stroke({ width: 1, color: edge, alpha: (estimate ? 0.5 : 0.9) * bAlpha });
      sp.badge.alpha = 1;
      sp.badgeText.text = badgeStr;
      sp.badgeText.visible = true;
      sp.badgeText.position.set(bx, by);
      sp.badgeText.style.fill = estimate ? 0x9fb2c9 : 0xffffff;
      sp.badgeText.alpha = bAlpha;
    } else {
      sp.badgeText.visible = false;
    }

    // §comms-v4.1: the tunnel keeps this selectable bookmark visible. Its
    // world position was captured once on entry and is never refreshed by the
    // arrived dark stream; only a served exit event replaces it.
    sp.container.alpha = 1;
    sp.container.visible = true;

    return s;
  }

  update(state: ViewState): void {
    if (!state.galaxy) return;
    if (this.galaxy !== state.galaxy) this.setGalaxy(state.galaxy);

    // Advance any galaxy⇄system transition (camera push + crossfade), and decide
    // which scene(s) to draw this frame. Only one scene is "live" at rest; during
    // a transition BOTH draw so the crossfade reads.
    const { drawGalaxy, drawSystem } = this.tickTransition();

    if (drawGalaxy) {
      // §perf: the static systems/anchors geometry is "dirty" only when the camera
      // moved, a new View arrived (stateVersion, bumped by main.ts), or the
      // selection changed. On idle frames with none of these (and no animating
      // system), drawSystems skips its rebuild entirely — the objects are pooled
      // and their screen geometry is unchanged.
      const stateDirty = this.stateVersion !== this.lastStateVersion;
      const emplacementDirty = this.viewDirty
        || stateDirty
        || state.selectedEmplacementId !== this.lastSelEmplacement;
      const geomDirty = this.viewDirty
        || stateDirty
        || state.selectedShipId !== this.lastSelShip
        || state.selectedSystemId !== this.lastSelSystem
        || this.selectedBattleMarkerId !== this.lastSelMarker;
      const laneCommsSignature = state.emplacements
        .filter((e) => e.own !== false && e.relay_throw > 0)
        .map((e) => `${e.id}:${e.kind}:${e.pos.x}:${e.pos.y}:${e.relay_throw}`)
        .sort()
        .join("|");
      // Redraw the world-anchored background when the camera moves, and the
      // lane zones when owned relay infrastructure changes.
      if (this.viewDirty || laneCommsSignature !== this.laneCommsSignature) {
        this.drawBackground();
        this.drawLanes(state);
        this.laneCommsSignature = laneCommsSignature;
        this.viewDirty = false;
      }
      // Emplacements change when a construction/demolition completes, and their
      // selection ring changes locally. Neither should wait for a camera move.
      if (emplacementDirty) this.drawEmplacements(state);
      const nowMs = performance.now();
      // Measure extrapolation in SIM time against the served snapshot. Because
      // the render clock remains continuous when a new View advances simTime,
      // the snapshot's position step and this dt reset cancel for steady motion.
      const dt = Math.max(0, Math.min(liveSimTime(nowMs) - state.simTime, MAX_EXTRAPOLATE_S));
      // The FRAME delta, which is a different quantity from `dt` above (that one
      // is the age of the last view). Position smoothing needs how long since the
      // last frame, or its time constant would vary with snapshot staleness.
      this.frameDt = this.lastFrameMs > 0 ? Math.min((nowMs - this.lastFrameMs) / 1000, 0.25) : 1 / 60;
      this.lastFrameMs = nowMs;

      // §hyperspace: the sensor bubble is NO LONGER DRAWN.
      //
      // A single ring was always a simplification — detection is
      // `bubble × signature`, so a quiet raider is only caught at 0.4× that
      // radius while a big fleet at speed is caught well outside it. The circle
      // therefore claimed a certainty it never had, in both directions, and the
      // speed-signature work will widen that gap further. Same reasoning that
      // retired the uncertainty cones: a shape that lies is worse than no shape.
      //
      // Coverage still governs everything it always did — it is simply reported
      // as a NUMBER on the Sensor Array that projects it, rather than drawn as a
      // boundary the player can misread as a guarantee.
      // this.drawSensorCoverage(state, dt);
      this.drawSystems(state, geomDirty);
      this.drawHubBody();
      this.drawRoutes(state);
      this.drawAnchors(state);
      this.drawCommandCenter(state);

      for (const sp of this.ghosts.values()) sp.seen = false;
      // §perf: one ghost-by-id map built per frame, shared by the draw paths that
      // used to each run their own O(n) find() (drawIntercepts).
      const ghostById = new Map<string, GhostView>();
      for (const gh of state.ghosts) ghostById.set(gh.id, gh);
      const screenById = new Map<string, { x: number; y: number }>();
      const orderFleetIds = new Set<string>();
      // §one-battle-one-icon: a fleet ENGAGED in a visible battle has its whole
      // map marker SUPPRESSED (sprite, heading hint, uncertainty cone, ownership
      // pip, count badge, echo badge) — the single battle icon carries the state.
      // Per the observer's LIGHT: `state.battles` is already light-gated, so a
      // distant observer whose retarded view still shows pre-battle fleets sees
      // them converge normally until the battle's light arrives. Its participant
      // ids are exactly the ghosts revealed at the site, so this never hides a
      // fleet the icon doesn't represent.
      const engaged = new Set<string>();
      for (const b of state.battles) for (const p of b.participants) engaged.add(p);
      // §dock: BERTHED hulls are not drawn on the star chart. A docked ship
      // belongs to the system view — drawing it here is what buried systems
      // under stacks of overlapping sprites and forced the hit-radius caps
      // below. Nothing is concealed: the same ghosts become the berth counts on
      // the system panel, so the information moves rather than disappearing.
      //
      // TWO ESCAPES, both deliberate. A fleet can only be berthed at the hub or
      // at ground its owner (or an ally) holds — so anything sitting on a
      // rival's world is blockading, besieging or invading, and keeps its
      // sprite. And a berth under BLOCKADE keeps drawing regardless: decluttering
      // must never become concealment of an attack on your own dock.
      const besieged = new Set<string>(
        state.systems.filter((s) => s.blockade !== null).map((s) => s.id),
      );
      for (const ghost of state.ghosts) {
        if (engaged.has(ghost.id)) {
          const sp = this.ghosts.get(ghost.id);
          if (sp) { sp.seen = true; sp.container.visible = false; } // keep pooled, hidden
          continue; // not in screenById → no order line either
        }
        const sp0 = this.ghosts.get(ghost.id);
        if (ghost.docked && !besieged.has(ghost.docked)) {
          const sp = this.ghosts.get(ghost.id);
          if (sp) { sp.seen = true; sp.container.visible = false; } // keep pooled, hidden
          continue;
        }
        if (sp0) sp0.container.visible = true; // un-suppress a fleet that broke away
        orderFleetIds.add(ghost.id);
        const screen = this.drawGhost(ghost, state, dt);
        // A tunnel bookmark is not the moving receiver, so never bend the comet
        // onto it; the fog-safe solved/served fallback below remains in force.
        if (!ghostInTunnel(ghost)) screenById.set(ghost.id, screen);
      }
      // A ship is drawn only while the server is sending its ghost. A destroyed
      // ship's ghost flies on old light until its destruction light reaches this
      // player, then the server stops sending it and it vanishes here at the kill
      // site — the moment the player observes the destruction (§6). No hold.
      for (const [id, sp] of this.ghosts) {
        if (!sp.seen) {
          this.ghostsLayer.removeChild(sp.container);
          sp.container.destroy({ children: true });
          this.ghosts.delete(id);
        }
      }
      this.updateReacquisitions();

      this.drawOrders(state, orderFleetIds);
      this.drawIntercepts(state, ghostById);
      this.drawBattles(state);
      this.drawAftermath(state);
      this.drawCaptures(state);
      this.drawSignals(state, screenById, dt);
      // §perf: record what this frame's geometry was drawn against, so the next
      // frame can decide whether a rebuild is needed.
      this.lastStateVersion = this.stateVersion;
      this.lastSelShip = state.selectedShipId;
      this.lastSelSystem = state.selectedSystemId;
      this.lastSelEmplacement = state.selectedEmplacementId;
      this.lastSelMarker = this.selectedBattleMarkerId;
    }

    if (drawSystem) {
      // Ownership is the ONLY dynamic input, and it comes from the SAME light-
      // gated per-player view (state.systems) the galaxy map reads — so the
      // System View is fogged identically and leaks nothing hidden.
      const sid = this.systemScene.currentId();
      const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
      this.systemScene.update(dyn?.owner ?? null, state.playerId, performance.now());
    }
  }

  /// Advance the crossfade/camera-push. Autopilot derives transitionRaw from the
  /// established wall clock; wheel scrub eases it toward the accumulated wheel
  /// target. Everything below that progress source is the shared camera/alpha
  /// language, so switching drivers cannot create a visual dialect change.
  private tickTransition(): { drawGalaxy: boolean; drawSystem: boolean } {
    const tr = this.transition;
    if (!tr) return { drawGalaxy: this.mode.type === "galaxy", drawSystem: this.mode.type === "system" };

    const now = performance.now();
    let complete = false;
    if (tr.driver === "autopilot") {
      const elapsed = clamp01((now - tr.start) / TRANS_MS);
      this.transitionRaw = tr.dir === "in" ? elapsed : 1 - elapsed;
      complete = elapsed >= 1;
    } else {
      // A target separate from the rendered progress absorbs trackpad bursts and
      // makes discrete mouse-wheel ticks read as one continuous handoff.
      const dt = Math.min(Math.max((now - this.transitionLastMs) / 1000, 0), 0.1);
      const blend = 1 - Math.exp(-18 * dt);
      this.transitionRaw += (this.transitionTargetRaw - this.transitionRaw) * blend;
      if (Math.abs(this.transitionTargetRaw - this.transitionRaw) < 0.001) {
        this.transitionRaw = this.transitionTargetRaw;
      }
      complete = this.transitionRaw === this.transitionTargetRaw
        && (this.transitionRaw === 0 || this.transitionRaw === 1);
    }
    this.transitionLastMs = now;

    const raw = tr.dir === "in" ? this.transitionRaw : 1 - this.transitionRaw;
    const p = easeInOut(raw);
    // Camera motion follows the direction of travel; magnification follows the
    // semantic position in the band (0 galaxy → 1 system) so reversal is exact.
    const semanticP = easeInOut(this.transitionRaw);
    this.cx = tr.camFrom.cx + (tr.camTo.cx - tr.camFrom.cx) * p;
    this.cy = tr.camFrom.cy + (tr.camTo.cy - tr.camFrom.cy) * p;
    this.scale = tr.camFrom.scale + (tr.camTo.scale - tr.camFrom.scale) * p;

    if (tr.target === "system" && tr.focus) {
      // Keep magnifying after the nominal r=96 seam. Zoom about the star so its
      // base camera trajectory is unchanged while ships, buoys, and lanes spread.
      const focusScreen = this.worldToScreen(tr.focus);
      const overshoot = 1 + (GALAXY_OVERSHOOT - 1) * semanticP;
      this.scale *= overshoot;
      this.cx = focusScreen.x - tr.focus.x * this.scale;
      this.cy = focusScreen.y - tr.focus.y * this.scale;

      // INVARIANT: GALAXY STAR ≤ SYSTEM STAR ALWAYS. The two matched stars
      // dissolve at one anchor while planets/orbits/belts grow around the fixed
      // System View disk; the vignette remains outside `content`.
      const star = this.systemScene.starLayoutPosition();
      const trackedFocus = this.worldToScreen(tr.focus);
      const contentScale = SCHEMATIC_GROW_FROM + (1 - SCHEMATIC_GROW_FROM) * semanticP;
      this.systemScene.content.pivot.set(star.x, star.y);
      this.systemScene.content.position.set(trackedFocus.x, trackedFocus.y);
      this.systemScene.content.scale.set(contentScale);
      this.systemScene.setStarCounterScale(contentScale);
    }
    this.viewDirty = true; // the camera moved — the galaxy background must redraw

    if (tr.dir === "in") {
      this.galaxyRoot.alpha = 1 - clamp01((raw - 0.35) / 0.65);
      if (tr.target === "system") this.systemScene.root.alpha = clamp01((raw - 0.25) / 0.75);
    } else {
      this.galaxyRoot.alpha = clamp01((raw - 0.25) / 0.75);
      if (tr.target === "system") this.systemScene.root.alpha = 1 - clamp01((raw - 0.35) / 0.65);
    }

    if (complete) {
      if (tr.target === "system") this.systemScene.resetContentTransform();
      if (this.transitionRaw === 1) {
        const changedMode = tr.target === "system" && this.mode.type !== "system";
        this.mode = tr.target === "system"
          ? { type: "system", systemId: tr.id }
          : { type: "battle", battleId: tr.id };
        this.galaxyRoot.alpha = 0;
        this.galaxyRoot.visible = false; // semantic detail is live — stop drawing the galaxy
        if (tr.target === "system") {
          this.systemScene.root.visible = true;
          this.systemScene.root.alpha = 1;
          if (tr.driver === "scrub" && changedMode) {
            this.scrubEndpoint = { type: "system", systemId: tr.id };
          }
        }
      } else {
        const changedMode = tr.target === "system" && this.mode.type !== "galaxy";
        this.mode = { type: "galaxy" };
        this.galaxyRoot.alpha = 1;
        this.galaxyRoot.visible = true;
        if (tr.target === "system") {
          this.systemScene.root.visible = false;
          this.systemScene.root.alpha = 1;
          if (tr.driver === "scrub" && changedMode) this.scrubEndpoint = { type: "galaxy" };
        }
        this.scrubGalaxyCam = null;
        if (tr.target === "system") this.systemFocus = null;
      }
      const endpointCam = tr.dir === "in"
        ? (this.transitionRaw === 1 ? tr.camTo : tr.camFrom)
        : (this.transitionRaw === 1 ? tr.camFrom : tr.camTo);
      this.cx = endpointCam.cx;
      this.cy = endpointCam.cy;
      this.scale = endpointCam.scale;
      this.transition = null;
    } else {
      this.galaxyRoot.visible = true;
    }
    return { drawGalaxy: true, drawSystem: tr.target === "system" };
  }

  /// Draw the OUTBOUND command signal (server-timed; we only place it at its
  /// interpolated `pOut`): the violet comet of an order in flight, command center
  /// → ship. This is the ONE thing the map can't show — your command crossing
  /// space, not yet arrived. The ship's REACTION needs no signal: it's seen
  /// directly on the map (in delayed light) when the ghost changes course. So
  /// there is no inbound/response leg, and raid results are a notification only.
  private drawSignals(state: ViewState, screenById: Map<string, { x: number; y: number }>, dt: number): void {
    const g = this.signalsGfx;
    g.clear();
    if (!state.commandCenter) return;
    const cc = this.worldToScreen(state.commandCenter);
    for (const text of this.commandSignalLabels.values()) text.visible = false;
    const liveSignalIds = new Set(state.commandSignals.map((signal) => signal.orderId));

    // OUTBOUND only: a violet comet, command center → ship. No return leg (the
    // ship's reaction is seen on the map), and no inbound result rings (a raid
    // outcome is seen on the map + a notification) — only what the map can't show.
    for (const sig of state.commandSignals) {
      // Aim at the SAME eased position drawn for the ship this frame. Using the
      // raw served ghost here made every new View move the final leg in a step,
      // even though drawGhost deliberately spreads that correction over several
      // frames; the comet therefore jittered beside an otherwise smooth ship.
      let gp = screenById.get(sig.shipId);
      if (!gp) {
        // Docked and battle-suppressed fleets have no rendered glyph. A tunnel
        // does have a bookmark, but it is not a receiver position: its comet
        // ends at the fog-safe solved meeting point when the route carries one;
        // a direct beyond-wire signal has no drawable hops, so it ends at the
        // last-served position. Neither case bends toward invented motion.
        const ghost = state.ghosts.find((x) => x.id === sig.shipId);
        if (!ghost) continue;
        const solved = ghostInTunnel(ghost) ? sig.hops[sig.hops.length - 1]?.pos : undefined;
        const servedPinned = ghost.own && this.servedStreamPinned(ghost, state.simTime);
        gp = this.worldToScreen(solved ?? (ghost.own && (!ghost.in_comms || servedPinned)
          ? ghost.pos
          : { x: ghost.pos.x + ghost.vel.x * dt, y: ghost.pos.y + ghost.vel.y * dt }));
      }

      const p = Math.max(0, Math.min(1, sig.pOut));
      const focusing = state.selectedOrderId !== null;
      const selected = state.selectedOrderId === sig.orderId;
      const emphasis = focusing ? (selected ? 1 : 0.35) : 1;
      // §buoys: the comet traces the RELAY PATH the order actually flies —
      // screen-space waypoints from cc through each hop, the LAST leg bent
      // onto the live, rendered ghost so arrival lands on the visible ship as
      // its delayed light is smoothly reconciled. `fracs` are the server's own hop times, so the comet
      // sprints along lanes and crawls the warp gaps. No hops = straight run.
      const pts: { x: number; y: number }[] = [cc];
      const fracs: number[] = [0];
      if (sig.hops.length >= 2) {
        for (let i = 0; i < sig.hops.length - 1; i++) {
          pts.push(this.worldToScreen(sig.hops[i].pos));
          fracs.push(sig.hops[i].frac);
        }
      }
      pts.push(gp);
      fracs.push(1);
      // The head: find the segment containing `p` and interpolate inside it.
      let seg = 1;
      while (seg < fracs.length - 1 && fracs[seg] < p) seg++;
      const f0 = fracs[seg - 1];
      const f1 = fracs[seg];
      const lt = f1 > f0 ? (p - f0) / (f1 - f0) : 1;
      const hx = pts[seg - 1].x + (pts[seg].x - pts[seg - 1].x) * lt;
      const hy = pts[seg - 1].y + (pts[seg].y - pts[seg - 1].y) * lt;
      const d = norm(pts[seg].x - pts[seg - 1].x, pts[seg].y - pts[seg - 1].y);
      // The traversed path behind the head, dashed along every completed leg.
      for (let i = 1; i < seg; i++) {
        dashedLine(g, pts[i - 1].x, pts[i - 1].y, pts[i].x, pts[i].y, 6, 7);
      }
      dashedLine(g, pts[seg - 1].x, pts[seg - 1].y, hx, hy, 6, 7);
      g.stroke({
        width: selected ? 1.8 : 1,
        color: COL_COMMAND,
        alpha: (selected ? 0.45 : 0.16) * emphasis,
      });
      for (let k = 1; k <= 4; k++) {
        g.circle(hx - d.x * k * 6, hy - d.y * k * 6, 4.4 - k * 0.8).fill({ color: COL_COMMAND, alpha: (0.42 - k * 0.08) * emphasis });
      }
      g.circle(hx, hy, selected ? 15 : 12).fill({ color: COL_COMMAND, alpha: (selected ? 0.2 : 0.12) * emphasis });
      g.circle(hx, hy, selected ? 6 : 5).fill({ color: COL_COMMAND, alpha: 0.98 * emphasis });
      arrowhead(g, hx + d.x * 6, hy + d.y * 6, d.x, d.y, selected ? 10 : 9, COL_COMMAND, 0.98 * emphasis);
      if (sig.beyondComms && seg === fracs.length - 1) {
        const text = this.commandSignalLabel(sig.orderId);
        text.visible = true;
        text.position.set(hx + 12, hy - 11);
        text.alpha = 0.78 * emphasis;
      }
    }
    for (const [id, text] of this.commandSignalLabels) {
      if (liveSignalIds.has(id)) continue;
      this.signalsLayer.removeChild(text);
      text.destroy();
      this.commandSignalLabels.delete(id);
    }
  }
}

function norm(dx: number, dy: number): { x: number; y: number } {
  const len = Math.hypot(dx, dy);
  return len < 1e-6 ? { x: 0, y: 0 } : { x: dx / len, y: dy / len };
}

// A small filled triangle at (x,y) pointing along (dx,dy).
function arrowhead(g: Graphics, x: number, y: number, dx: number, dy: number, size: number, color: number, alpha: number): void {
  const px = -dy;
  const py = dx; // perpendicular
  const tipX = x + dx * size;
  const tipY = y + dy * size;
  const blX = x - dx * size * 0.2 + px * size * 0.7;
  const blY = y - dy * size * 0.2 + py * size * 0.7;
  const brX = x - dx * size * 0.2 - px * size * 0.7;
  const brY = y - dy * size * 0.2 - py * size * 0.7;
  g.poly([tipX, tipY, blX, blY, brX, brY]).fill({ color, alpha });
}


function dashedLine(g: Graphics, x1: number, y1: number, x2: number, y2: number, dash: number, gap: number): void {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy);
  if (len < 1) return;
  const ux = dx / len;
  const uy = dy / len;
  let d = 0;
  while (d < len) {
    const a = d;
    const b = Math.min(d + dash, len);
    g.moveTo(x1 + ux * a, y1 + uy * a).lineTo(x1 + ux * b, y1 + uy * b);
    d += dash + gap;
  }
}

function dashedCircle(g: Graphics, x: number, y: number, radius: number, segments: number): void {
  for (let i = 0; i < segments; i += 2) {
    const a0 = (i / segments) * Math.PI * 2;
    const a1 = ((i + 1) / segments) * Math.PI * 2;
    g.moveTo(x + Math.cos(a0) * radius, y + Math.sin(a0) * radius);
    g.arc(x, y, radius, a0, a1);
  }
}

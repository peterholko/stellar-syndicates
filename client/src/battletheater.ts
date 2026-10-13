// §theater — BATTLE THEATER: LOD2 hulls & weapon FX over truth keyframes.
//
// The theater is a REPLAYER. It renders what the record says happened — the
// tactical engine's keyframed positions, salvo summaries, and exact death
// events — and can never influence anything. Standing laws:
//
//   * DETERMINISM — all cosmetic placement (which sprite fires, miss offsets,
//     debris angles) comes from a seeded PRNG on `(battleId, round)`, never
//     `Math.random()`: two independent renders of the same record are the
//     identical scene, and scrubbing back replays identically (each round's
//     FX schedule is re-derived from its seed, no accumulated state).
//   * FIDELITY — keyframes ship at participant fidelity only; this canvas
//     simply never opens for bucket viewers (they keep the column arena).
//   * ISOLATION — the theater owns its own small Pixi Application inside the
//     shell-owned theater overlay (the map's renderer is untouched), renders only
//     while the viewer is open, and pauses when the tab is hidden.
//
// Data reality (what the record actually carries, and what we derive):
//   * `KeyframeView.ships` carry battle-local ids (old records fall back to
//     nearest-neighbour matching); headings come from position deltas, and
//     a representative's ×N badge is `side count of kind ÷ shown of kind`.
//   * Gunfire is resolved shot evidence: one bolt per shown attempt, misses
//     pass clear, impacts require damage. Legacy records can illustrate known
//     hull loss but never manufacture successful fire on a zero-damage round.
//   * torpedoes are ONE centroid salvo per side with a live count → the
//     theater expands them into individual cosmetic arcs; the round-to-round
//     count drop budgets how many arcs resolve (hit or intercepted).

import { Application, Assets, Container, Graphics, Rectangle, Sprite, Text, TextStyle, Texture } from "pixi.js";
import type { BattleRecordView, KeyframeView, PlayerId, ShipKind } from "./protocol";
import { hashId, mulberry32 } from "./prng";
import { starAnchor, starTypeFor, starIconUrl, starVisualRatio } from "./stars";
import { STAR_TINT, SystemViewScene, type VisualSystem } from "./systemview";
import { battleBackdropPlan, battleSceneryFrame, battleSystemScenery, type BattleBackdropPlan, type BattleScenerySource } from "./battlebackdrop";
import { glowSpriteTexture, terminatorTexture, towardStarRotation, drawRings } from "./systembodyfx";
import { stationArtUrl, stationName } from "./core/derive/station";
import { planetArtUrl, planetArtwork } from "./planetart";
import { COL_OWN as TINT_OWN, COL_OTHER as TINT_FOE } from "./render";
import { arrivedGunfire, matchBattleFrames, missedEndpoint } from "./battlefire";
import { COMBAT_HULL_LENGTH_SCALE, shipArtwork, shipArtUrl } from "./shipart";
import { BattleModels } from "./battle-models";
import { damageVents, enginePlumes, engineThrust, gunPoints, hullHalfExtents, hullOnFire, MAX_SPEED, noise1, PD_RADIUS, PD_RADIUS_HEAVY,
  smokeVents, TORP_SPEED, TURN_RATE, turnToward, wrapAngle, type Plume } from "./battlefx";
import { artOpacity, createFxTextures, FxSpritePool, paintDebrisAtlas, type DebrisAtlas, type FxTextures } from "./battlefxsprites";
import type { BattleBurst, BattleModelFrame, BattleWreckPiece } from "./battle-model-policy";
import { blastClass, breachDelay, charOf, debrisCutFraction, debrisCuts, drift, emberColor, fragments, MAX_WRECKS, mix,
  pieceMotion, PLATE_SPAN, shockPush, shockRadius, SLIVER_SPAN, WRECK_PIECES, wreckCuts, type Fragment,
  type PieceMotion } from "./battlewrecks";

// --- Geometry & scale ---------------------------------------------------------

/// Battle-local arena half-extent the theater frames (arena ring 1000 +
/// withdraw margin — mirrors the sim's WITHDRAW_EXIT_RADIUS 1400).
const ARENA_R = 1000;
const VIEW_R = 1500;
/// Logical canvas size. Desktop keeps the original 1068×672 stage; portrait
/// viewers pass their measured stage at attach time. Keeping these mutable is
/// deliberate: Pixi's application survives shell/viewport changes, so resizing
/// must not recreate the WebGL context or discard the camera.
const DEFAULT_CANVAS_W = 1068;
const DEFAULT_CANVAS_H = 672;
let CANVAS_W = DEFAULT_CANVAS_W;
let CANVAS_H = DEFAULT_CANVAS_H;
/// Arena → screen scale at zoom 1: fit the withdraw margin into the canvas
/// SHORT axis. The arena is radial, so using either width or height alone can
/// clip it in portrait. The CAMERA multiplies this (zoom, pan, auto-fit).
let SCALE = Math.min(CANVAS_W, CANVAS_H) / (2 * VIEW_R);

export interface TheaterViewport {
  width: number;
  height: number;
  maxFps?: number;
}

/// Client mirror of the sim's hull masses (also mirrored in main.ts — keep in
/// step with crates/sim/src/ship.rs::hull_mass).
const MASS: Record<ShipKind, number> = {
  tiny_freighter: 1500, small_freighter: 2500, large_freighter: 8000, heavy_freighter: 14000, bulk_freighter: 24000,
  convoy: 4500, raider: 200, corvette: 800, colony: 6000, scout: 80, survey_probe: 40, mining_barge: 3500,
  mining_belt_station: 12000,
  destroyer: 2000, cruiser: 4000, battleship: 8000, dreadnought: 16000, titan: 32000,
  // §ground: the troop transport — a fat, unarmed hull.
  transport: 7000,
  builder: 2500, // §emplacements: the crane — mirrors sim hull_mass
  // §TCA: the Authority's carrier — never a combatant, but the type is total.
  freighter: 6000,
};
const KIND_LABEL: Record<ShipKind, string> = {
  tiny_freighter: "Tiny Freighter", small_freighter: "Small Freighter", large_freighter: "Large Freighter", heavy_freighter: "Heavy Freighter", bulk_freighter: "Bulk Freighter",
  builder: "Construction Ship",
  convoy: "Medium Freighter", raider: "Interceptor", corvette: "Corvette", colony: "Colony Ship", scout: "Scout", survey_probe: "Survey Probe", mining_barge: "Mining Barge",
  mining_belt_station: "Mining Belt Station",
  destroyer: "Destroyer", cruiser: "Cruiser", battleship: "Battleship", dreadnought: "Dreadnought", titan: "Titan",
  transport: "Troop Transport",
  freighter: "Authority Freighter",
};

/// The approved combat silhouette ladder is shared with galaxy close-ups.
/// Noncombat hulls keep their old mass-based presentation; this changes no
/// tactical positions, weapon ranges, hit rolls or collision geometry.
function spritePx(kind: ShipKind): number {
  const ratio = COMBAT_HULL_LENGTH_SCALE[kind];
  if (ratio !== undefined) return 10 * ratio;
  return Math.max(10, Math.min(64, 7.0 * Math.pow((MASS[kind] ?? 800) / 100, 0.4)));
}

// Team identity: the map's own constants (imported from render.ts — no
// invented palette), applied as engine/rim glow; the art stays natural.

// --- Art: one resolver, art-or-fallback ---------------------------------------

/// Individual hulls use the same approved masters as the map and panels.
/// The procedural fallback remains visible while a native texture is loading.
const STATION_ART = "/art/celestial_sprites/mining_station.png";
// Pirate hull culture: one deterministic silhouette per engagement. Tactical
// keyframes carry a side and kind but no fleet id, so the battle id is the
// stable cosmetic seed; this never changes combat truth or replay timing.
const PIRATE_RAIDER_ART = [
  { url: "/art/ship_sprites/privateer_raider_ship.png", calib: 0.73 },
  { url: "/art/ship_sprites/npc-contractors/pirate_corsair.png", calib: 0.70 },
  { url: "/art/ship_sprites/npc-contractors/pirate_boarding_raider.png", calib: 0.70 },
] as const;
const pirateRaiderArt = (battleId: string) =>
  PIRATE_RAIDER_ART[hashId(`${battleId}:pirate-hull`) % PIRATE_RAIDER_ART.length];

/** The served side identifies the hull's owner: pirates share the `raider`
 * combat class with corporate Interceptors, not their name or artwork. Keep
 * the force header, hover labels and canvas on the same deterministic art. */
export function theaterShipAppearance(
  record: BattleRecordView, side: number, kind: ShipKind, pirateId: PlayerId | null,
  canvasPx = 256, resolution = 1,
): { label: string; url: string; calib: number; anchor?: readonly [number, number] } {
  const pirate = kind === "raider" && pirateId !== null && record.sides[side]?.corp === pirateId;
  return pirate
    ? { label: "Privateer", ...pirateRaiderArt(record.id) }
    : { label: KIND_LABEL[kind], url: shipArtUrl(kind, canvasPx, resolution),
      calib: shipArtwork(kind).calib, anchor: shipArtwork(kind).anchor };
}

const texCache = new Map<string, Texture | null>();
const texPending = new Set<string>();
/// THE resolver: art if loaded, null → procedural fallback. Load is fired on
/// first ask and swaps in when it lands (the map renderer's idiom).
function resolveTexture(url: string): Texture | null {
  const hit = texCache.get(url);
  if (hit !== undefined) return hit;
  if (!texPending.has(url)) {
    texPending.add(url);
    Assets.load(url).then(
      (t: Texture) => {
        t.source.autoGenerateMipmaps = true;
        t.source.scaleMode = "linear";
        texCache.set(url, t);
        // A paused replay / held light frontier may never advance a round.
        // Redress it when art lands instead of keeping the tiny placeholder.
        if (st) st.windowKey = "";
      },
      () => texCache.set(url, null), // missing art → permanent fallback
    );
  }
  return null;
}
// --- Scene types ----------------------------------------------------------------

/// One tracked representative on screen (pooled — never allocated per frame).
interface ShipVis {
  cid?: number;
  /// Screen position + scale; its vertical squash lays flat art on the plane.
  root: Container;
  /// The hull's heading on the battlefield plane (inside the squash).
  spin: Container;
  sprite: Sprite;
  body: Graphics; // procedural silhouette fallback
  glow: Graphics; // faint team-colour disc
  chrome: Container; // count/name labels above the shared 3D surface
  badge: Text; // ×N representative count
  plate: Text; // Titan nameplate / platform label
  hull: Container; // screen-space readout, not rotated or dimmed with the ship art
  hullBar: Graphics;
  hullOffsetPx: number;
  engines: Container; // engine flames, inside the hull's transform and under its art
  flames: Sprite[]; // two per plume: nozzle glow, then flame
  plumes: Plume[]; // ship-local plume anchors for this hull at this size
  guns: [number, number][]; // ship-local gun barrels (the 3D hull's sockets)
  vents: [number, number][]; // ship-local damage vents, stable for the battle
  halfW: number; // footprint half extents, ship-local px
  halfL: number;
  fxSeed: number; // stable per hull across windows (smoke, flicker)
  modelPx: number;
  modelEligible: boolean;
  inUse: boolean;
  // matched track for the active window:
  x0: number; y0: number; x1: number; y1: number;
  hdg0: number; // facing at window ENTRY and EXIT (facingTable) — rotation
  hdg: number; // is a PURE function of (record, round, frac): hdg0→hdg over frac.
  vx0: number; vy0: number; // previous window's displacement (arena units/round)
  vx2: number; vy2: number; // next window's displacement, once its light has arrived
  speed: number; // along the curved track this frame (arena units/round)
  burn: number; // acceleration along the heading this frame (units/round²)
  hitTimes: number[]; // evidenced impacts on this hull this window (frac)
  deathT: number | null; // bound recorded death time within the window
  side: number;
  kind: ShipKind;
  plat: boolean;
  /** §orbital-defense: a station hardpoint, with its station's level (0 = not). */
  station: number;
  hp: number;
  hpStart: number;
  hpEnd: number;
  hpT: number; // last evidenced impact in this already-arrived window
  reps: number; // hulls this sprite stands for
  seed: number; // per-ship cosmetic seed (idle drift phase)
  entered: boolean; // newly committed this window (wave arrival) → fade in
  exiting: boolean; // unmatched in the next frame (died/withdrew) → fade out
}

/// The FX SCHEDULE for one window — derived ONCE per (battleId, round) from
/// record data + the cosmetic PRNG, then rendered statelessly against the
/// transport clock. Scrubbing back re-derives the identical schedule.
interface FxWindow {
  /// Resolved beam attempts. `heavy` = capital emitter
  /// (charge-up + thicker, longer-held line; the Lance Array variant when
  /// that module lands). `glint` = target is Reflective-fitted (mirror-flash
  /// deflection instead of a full bloom).
  beams: { t: number; from: number; to: number; w: number; heavy: boolean; glint: boolean; miss: boolean; missSign: number }[];
  /// Driver tracer streaks with short visible flight; `miss` tracers pass
  /// close and carry on (miss offsets are cosmetic; hit/miss is resolved).
  /// `spall` = target is Whipple-fitted (shattered-armor debris puffs
  /// instead of clean sparks).
  tracers: { t: number; from: number; to: number; miss: boolean; missSign: number; spall: boolean; strength: number }[];
  /// Torpedo arcs — the centerpiece motion. Expanded from the per-side salvo
  /// summary: each arc curves from the salvo origin toward its target across
  /// the window. outcome: 'fly' persists past the window; 'hit' detonates at
  /// tEnd; 'flak' dies to point-defense short of the target at tEnd.
  arcs: { side: number; x0: number; y0: number; cx: number; cy: number; to: number; tEnd: number; outcome: "fly" | "hit" | "flak" | "fade" }[];
  /// PD tracer fans: which defending ships screen this window (indices).
  pdShips: number[];
  /// Exact deaths from the record, timed within the window by their step.
  deaths: { t: number; x: number; y: number; kind: ShipKind; side: number; cls: 0 | 1 | 2 | 3; shipIdx: number | null; cid: number | null }[];
  /// Hulls that rammed wreckage (rules v9), at window time `t`: the contact
  /// point (arena) and the hull damage.
  rams: { t: number; ship: number | null; x: number; y: number; damage: number }[];
  /// Recorded torpedoes (frames that carry them one by one); `arcs` stays the
  /// fallback for older frames that only summarize salvos.
  torps: TorpTrack[];
  /// Rules v11: interceptors holding a hull that is trying to leave, and stasis
  /// webs slowing their targets (ship indices in this window).
  tackles: { from: number; to: number }[];
  webs: { from: number; to: number }[];
}

/// One recorded torpedo across a window. It flies its real path at its real
/// speed: from (x0, y0) at the window start to (x1, y1) at its end, or to where
/// it ended at `tEnd`. A launch rides its launcher (`from`) from `t0` until the
/// window ends. `to`, `by` and `from` are ship indices in this window (null when
/// that hull is not shown).
interface TorpTrack {
  id: number;
  side: number;
  to: number | null;
  by: number | null;
  from: number | null;
  x0: number; y0: number;
  x1: number; y1: number;
  t0: number;
  tEnd: number;
  end: "fly" | "launch" | "hit" | "miss" | "intercepted" | "dry" | "fade" | "wreck";
  /// Trail already behind it at the window start (arena units).
  trail0: number;
}

interface TheaterState {
  rec: BattleRecordView;
  pirateId: PlayerId | null;
  round: number; // active window = frames[round] → frames[round+1]
  frac: number;
  live: boolean;
  ships: ShipVis[];
  fx: FxWindow | null;
  withdrawFrom: [number, number]; // first round each side runs (Infinity = never)
  windowKey: string; // `${recId}:${round}` — rebuild tracks when it changes
}

// --- The one theater instance ------------------------------------------------------

let app: Application | null = null;
let holder: HTMLDivElement | null = null;
let tooltip: HTMLDivElement | null = null;
// The ship under the viewer's pointer (a held finger on touch screens): the
// only hull meter on screen. Pixi's pointer over/out events keep it current,
// including when a ship moves under a still pointer.
let hoverShip: ShipVis | null = null;
const HOVER_MIN_PX = 12; // a distant interceptor still has a pointable target
let layers: {
  backdrop: Container;
  debris: Container;
  wrecks: Container;
  ships: Container;
  fx: Container;
  ui: Container;
} | null = null;
let st: TheaterState | null = null;
let shipPool: ShipVis[] = [];
let backdropKey = ""; // rebuilt only when the record changes
/// Rules v14: the defended world a battle is fought at, on the battlefield
/// plane (arena units), where hulls round it and torpedoes burst on it. The
/// sprite follows the tactical camera; it replaces the screen-space world
/// beneath an in-orbit fight.
let arenaWorldBody: { x: number; y: number; r: number } | null = null;
let arenaWorld: Container | null = null;
let arenaWorldDisc: Sprite | null = null;
let backdropSystem: SystemViewScene | null = null;
let backdropVisual: VisualSystem | null = null; // frozen served astronomy, never live administration
let backdropPlan: BattleBackdropPlan | null = null; // the place the fight stands, frozen at open
const SYSTEM_BACKDROP_ALPHA = 0.42; // Tunable: recognizable scenery below ships/weapon effects
let sceneClock = 0; // seconds since open — drives idle drift only (cosmetic)
/// After a concluded battle's last record, players keep the replay running this
/// many rounds so the final deaths' breaches, wrecks and debris play out (the
/// theater holds the last keyframe for any time past the record).
export const THEATER_EPILOGUE_ROUNDS = 3;

// Persistent surfaces — redrawn each frame, never re-allocated (the budget
// law: pooled surfaces and sprites, no per-frame objects).
let debrisG: Graphics | null = null;
let fxTex: FxTextures | null = null; // painted once; null without a canvas (tests)
let fxLight: FxSpritePool | null = null; // additive: beams, flames, sparks, flashes
let fxSmoke: FxSpritePool | null = null; // normal blend, beneath the light
let fxDebris: FxSpritePool | null = null; // debris in the battlefield plane, beneath every hull
let banner: HTMLDivElement | null = null;
let bannerKey = "";
let cameraHint: HTMLDivElement | null = null;
let battleModels: BattleModels | null = null;
let modelFx: ReadonlySet<string> = new Set();
let modelWrecks: ReadonlySet<string> = new Set(); // wreck sections the 3D renderer drew
let reducedMotion = false;
/// The whole viewer (2D and 3D) renders at one cap, so hulls, labels and
/// weapon paths always advance together: 60 fps on a desktop stage (fast
/// hulls zoomed in move half as far per frame as at 30), 30 where the shell
/// asks for it (phones).
const FAST_FPS = 60;
const SLOW_FPS = 30;
let preferredFps = SLOW_FPS;
let targetFps = SLOW_FPS;
// Degradation ladder: rolling fps estimate. First rung: a stage that can't
// hold 60 fps drops to 30 (a steady frame beats a fast one), trying 60 again
// later. Then effects go: tier 0 full · 1 no miss-tracers · 2 thinned drivers
// · 3 reduced debris. Torpedo arcs, flak, deaths, and mitigation feedback are
// NEVER dropped (they carry information).
let fpsEma = SLOW_FPS;
let perfTier = 0;
/// Scene time (s) of the next try at the preferred cap, and the wait before
/// the one after (doubling with each failed try).
let fastRetryAt = 0;
let fastRetryWait = 20;
/// Opening a battle loads its hull models; its first frames don't count.
const PACE_WARMUP = 1.5;

// --- Camera (a VIEW control — never scene content; the hash ignores it) ----
// It opens on the arena overview (the ring filling the stage — with §arena
// discipline the fight stays inside it), following one of the player's
// hulls (see followCamera): zooming in closes on that hull. Dragging frees
// the camera, clicking a hull follows it, double-click resets to the
// overview and resumes following. Zoom is always the player's.
const DEFAULT_ZOOM = 1.3; // the arena ring + margin fills the stage
let camX = 0;
let camY = 0;
let camZoom = DEFAULT_ZOOM;
let dragging = false;
let dragLast: [number, number] | null = null;
const activePointers = new Map<number, [number, number]>();
let pinchStart: { distance: number; zoom: number; worldX: number; worldY: number } | null = null;
const CAM_ZOOM_MIN = 0.6;
// Inspection zoom: a light Interceptor reaches ~83px instead of ~36px. Keep
// the opening overview unchanged; both wheel and pinch use this ceiling.
const CAM_ZOOM_MAX = 12;

/// The followed hull (battle-local cid; null = choose one) and whether the
/// camera follows at all (a drag frees it). A change of hull glides there.
let following = true;
let followCid: number | null = null;
const FOLLOW_GLIDE_S = 0.6;
let followGlide: { x: number; y: number; t: number } | null = null;
/// The press under way has moved past DRAG_START_PX: a drag, not a click.
/// Below that a click keeps its wobble (a trackpad tap moves a pixel or two).
let dragMoved = false;
let pressAt: [number, number] = [0, 0];
const DRAG_START_PX = 5;
/// A click picks the nearest hull within this reach (or its own size): a
/// distant interceptor is only a few pixels.
const PICK_PX = 22;
let coarsePointer = false;
let hintKey = "";

// --- View angle (a VIEW control like the camera — never scene content) -----
// The battlefield is a plane seen at a slight angle: its vertical axis is
// foreshortened by THEATER_TILT (cos of a ~37° inclination), milder than the
// System View's 0.6 because formations are the information here. Positions,
// timing, hits and the hash are untouched; only the screen mapping changes.
// The player may switch to a straight top-down view (remembered per viewer).
const THEATER_TILT = 0.8; // Tunable: 1 = top-down
/// Mirror the 3D defaults (BATTLE_MODEL_TILT, visitBattleBurst's ring tilt):
/// the top-down pose, kept local to avoid a value import here. It leans the
/// same way as the angled view (seen from the lower edge), so a toggle eases
/// one steady lean, never rolling hulls through flat to the other side.
const TOPDOWN_HULL_PITCH = -Math.PI / 10;
const TOPDOWN_RING_TILT = -0.18;
const VIEW_KEY = "ss_theater_view";
function loadAngledView(): boolean {
  try { return localStorage.getItem(VIEW_KEY) !== "topdown"; } catch { return true; }
}
let viewAngled = loadAngledView();
/// The current foreshortening, eased toward the chosen view on a toggle.
let tiltY = viewAngled ? THEATER_TILT : 1;
/// True while the camera rests on the arena overview: it refits as the view
/// angle eases or the stage resizes. Any player zoom/pan releases it.
let cameraOverview = false;
let viewToggle: HTMLButtonElement | null = null;

const sx = (x: number) => CANVAS_W / 2 + (x - camX) * SCALE * camZoom;
const sy = (y: number) => CANVAS_H / 2 + (y - camY) * SCALE * camZoom * tiltY;
/// Screen → arena (for zoom-toward-cursor and drag panning).
const ax = (px: number) => camX + (px - CANVAS_W / 2) / (SCALE * camZoom);
const ay = (py: number) => camY + (py - CANVAS_H / 2) / (SCALE * camZoom * tiltY);
/// A heading on the battlefield plane, as drawn on screen.
const screenHeading = (h: number) => Math.atan2(Math.sin(h) * tiltY, Math.cos(h));
/// The plane's inclination, and how far the view has eased from top-down
/// (0) to the angled view (1).
const inclination = () => Math.acos(Math.min(1, tiltY));
const angledness = () => THEATER_TILT >= 1 ? 0 : Math.min(1, Math.max(0, (1 - tiltY) / (1 - THEATER_TILT)));
/// The 3D pose: angled, hulls and shock rings lie in a plane seen from its
/// lower (near) edge, and lower hulls sit nearer the camera; top-down keeps a
/// slight lean the same way. The two blend while the view eases (−18° to −37°
/// for hulls).
const hullPitch = () => -inclination() + TOPDOWN_HULL_PITCH * (1 - angledness());
const ringTilt = () => -inclination() + TOPDOWN_RING_TILT * (1 - angledness());
const depthSlope = () => Math.tan(inclination());

/// The overview zoom: the arena ring fills the stage. Angled, the plane is
/// shorter, so it may zoom in to refill the stage's height within its width.
function overviewZoom(): number {
  const widthRoom = (0.92 * CANVAS_W) / (2 * ARENA_R * SCALE * DEFAULT_ZOOM);
  return DEFAULT_ZOOM * Math.max(1, Math.min(1 / tiltY, widthRoom));
}

function resetCamera(): void {
  camX = 0;
  camY = 0;
  camZoom = overviewZoom();
  cameraOverview = true;
  following = true;
  followGlide = null;
}

/// The hull to follow by default: the player's heaviest still in the fight
/// (lowest id on a tie). None in a battle the player only watches.
function defaultFollow(): ShipVis | undefined {
  if (!st || st.rec.own_side === null) return undefined;
  let best: ShipVis | undefined;
  for (const v of st.ships) {
    if (!v.inUse || v.plat || v.exiting || v.side !== st.rec.own_side) continue;
    const mass = MASS[v.kind] ?? 0, bestMass = best ? MASS[best.kind] ?? 0 : -1;
    if (mass > bestMass || (mass === bestMass && (v.cid ?? 0) < (best?.cid ?? 0))) best = v;
  }
  return best;
}

/// The camera's centre held inside the overview's frame: at the opening zoom
/// that is the arena's centre (the whole battle in view); zoomed in, it can
/// reach any hull in the arena.
function clampToOverview(x: number, y: number): [number, number] {
  const z0 = overviewZoom();
  const roomX = Math.max(0, CANVAS_W / 2 / (SCALE * z0) - CANVAS_W / 2 / (SCALE * camZoom));
  const roomY = Math.max(0, CANVAS_H / 2 / (SCALE * z0 * tiltY) - CANVAS_H / 2 / (SCALE * camZoom * tiltY));
  return [Math.max(-roomX, Math.min(roomX, x)), Math.max(-roomY, Math.min(roomY, y))];
}

/// Centre on the followed hull at whatever zoom the player chose. When it
/// falls or leaves, follow the player's next heaviest, gliding over.
function followCamera(dt: number, frac: number): void {
  if (!st) return;
  let v = followCid !== null ? st.ships.find((s) => s.inUse && s.cid === followCid) : undefined;
  if (!v) {
    v = defaultFollow();
    if (!v) return;
    if (followCid !== null) followGlide = { x: camX, y: camY, t: 0 };
    followCid = v.cid ?? null;
  }
  const m = trackMotion(v, frac);
  let [x, y] = clampToOverview(m.x, m.y);
  if (followGlide) {
    followGlide.t = Math.min(1, followGlide.t + Math.max(0, dt) / FOLLOW_GLIDE_S);
    const k = followGlide.t * followGlide.t * (3 - 2 * followGlide.t);
    x = followGlide.x + (x - followGlide.x) * k;
    y = followGlide.y + (y - followGlide.y) * k;
    if (followGlide.t >= 1) followGlide = null;
  }
  camX = x;
  camY = y;
}

/// The stage's camera hint, naming the followed hull. Written only when it
/// changes (the transport re-attaches the theater every frame).
function syncCameraHint(): void {
  if (!cameraHint) return;
  const v = following && st && followCid !== null ? st.ships.find((s) => s.inUse && s.cid === followCid) : undefined;
  const who = v && st ? `${v.side === st.rec.own_side ? "your " : ""}${theaterShipAppearance(st.rec, v.side, v.kind, st.pirateId).label}` : "";
  const tap = coarsePointer ? "tap" : "click";
  const text = who ? `following ${who} · ${tap} another ship to switch · drag to look around`
    : coarsePointer ? "pinch zoom · drag pan · tap a ship to follow it · Reset camera resumes following"
    : "scroll zoom · drag pan · click a ship to follow it · double-click resumes following";
  if (text === hintKey) return;
  hintKey = text;
  cameraHint.textContent = text;
}

/// Select and follow the hull nearest a click, within PICK_PX of it or its
/// own drawn size, keeping the zoom.
function pickShip(ev: PointerEvent): void {
  if (!st || !app) return;
  const r = app.canvas.getBoundingClientRect();
  if (r.width < 2 || r.height < 2) return;
  pickAt(((ev.clientX - r.left) / r.width) * CANVAS_W, ((ev.clientY - r.top) / r.height) * CANVAS_H);
}

/// The same, at a point on the canvas.
function pickAt(mx: number, my: number): void {
  if (!st) return;
  let best: ShipVis | undefined, bestD = Infinity;
  for (const v of st.ships) {
    if (!v.inUse || v.cid === undefined || v.root.alpha <= 0.01) continue;
    const d = Math.hypot(v.root.position.x - mx, v.root.position.y - my);
    if (d <= Math.max(PICK_PX, v.modelPx * 0.55 * v.root.scale.x) && d < bestD) { best = v; bestD = d; }
  }
  if (!best) return;
  if (best.cid !== followCid || !following) followGlide = { x: camX, y: camY, t: 0 };
  followCid = best.cid ?? null;
  following = true;
}

/// The selected hull as the viewer shows it now: the one followed, or the
/// last one clicked after a drag freed the camera. Null when none is in view.
export interface TheaterShipInfo {
  cid: number;
  side: number;
  own: boolean;
  kind: ShipKind;
  label: string;
  art: string | null;
  /// Hull at the shown moment, 0–1.
  hull: number;
  /// Hulls this one stands for when a crowd is drawn thinned.
  reps: number;
  station: number;
  plat: boolean;
  /// The side's fits for this hull type (the record keeps fits per type).
  fits: string[][];
  flagship: string | null;
  status: "fighting" | "arriving" | "withdrawing" | "leaving" | "destroyed";
  following: boolean;
}

export function theaterSelection(): TheaterShipInfo | null {
  if (!st || followCid === null) return null;
  const v = st.ships.find((s) => s.inUse && s.cid === followCid);
  if (!v || v.cid === undefined) return null;
  const side = st.rec.sides[v.side];
  const appearance = theaterShipAppearance(st.rec, v.side, v.kind, st.pirateId);
  const status: TheaterShipInfo["status"] = v.deathT !== null && st.frac >= v.deathT ? "destroyed"
    : v.exiting && v.deathT === null ? "leaving"
    : st.round + st.frac >= st.withdrawFrom[v.side] ? "withdrawing"
    : v.entered && st.frac < 0.5 ? "arriving" : "fighting";
  return {
    cid: v.cid, side: v.side, own: v.side === st.rec.own_side, kind: v.kind,
    label: v.station ? `${stationName(v.station)} hardpoint` : v.plat ? "Defense Platform" : appearance.label,
    art: v.plat ? null : appearance.url, hull: v.hp, reps: v.reps, station: v.station, plat: v.plat,
    fits: (side?.loadouts ?? []).filter((l) => l.kind === v.kind && l.modules.length > 0).map((l) => [...l.modules]),
    flagship: v.kind === "titan" ? side?.flagship_name ?? null : null,
    status, following,
  };
}

/// Follow the selected hull again, or free the camera from it.
export function theaterFollow(on: boolean): void {
  if (on && !following) followGlide = { x: camX, y: camY, t: 0 };
  following = on && followCid !== null;
  if (!following) followGlide = null;
}

function syncViewToggle(): void {
  if (!viewToggle) return;
  viewToggle.textContent = "Top-down";
  viewToggle.setAttribute("aria-pressed", String(!viewAngled));
  viewToggle.title = viewAngled
    ? "Look straight down at the battlefield (best for reading formations)"
    : "Return to the angled view of the battlefield";
}

/// Switch between the angled and top-down views (eased; remembered).
export function theaterSetView(angled: boolean): void {
  viewAngled = angled;
  try { localStorage.setItem(VIEW_KEY, angled ? "angled" : "topdown"); } catch { /* per-viewer convenience only */ }
  syncViewToggle();
}

export function theaterViewAngled(): boolean {
  return viewAngled;
}

function configureViewport(viewport: TheaterViewport): boolean {
  const width = Math.max(240, Math.min(1600, Math.round(viewport.width)));
  const height = Math.max(240, Math.min(1200, Math.round(viewport.height)));
  if (width === CANVAS_W && height === CANVAS_H) return false;
  CANVAS_W = width;
  CANVAS_H = height;
  SCALE = Math.min(CANVAS_W, CANVAS_H) / (2 * VIEW_R);
  app?.renderer.resize(CANVAS_W, CANVAS_H);
  backdropKey = "";
  return true;
}

/// Allocation-free deterministic jitter in [0,1) — integer-hash based, for
/// per-frame cosmetics (PD fans, shake) where a full PRNG stream per frame
/// would allocate a closure (the budget law: never allocate per frame).
function jitter(a: number): number {
  let x = a | 0;
  x = Math.imul(x ^ (x >>> 15), 0x2c1b3c6d);
  x = Math.imul(x ^ (x >>> 12), 0x297a2d39);
  x ^= x >>> 15;
  return (x >>> 0) / 4294967296;
}

const clamp01 = (v: number): number => Math.max(0, Math.min(1, v));

/// Saturated laser identity: the viewer's side fires cold cyan-blue, the foe
/// hot red. A neutral observer uses attacker blue / defender red so crossings
/// remain readable without inventing ownership.
function sideLaserColor(side: number): number {
  if (!st || st.rec.own_side === null) return side === 0 ? TINT_OWN : TINT_FOE;
  return side === st.rec.own_side ? TINT_OWN : TINT_FOE;
}

let appInit: Promise<void> | null = null;
let theaterFailed = false; // WebGL/init failure → the SVG truth map takes over

/// Is the theater usable? (False after an init failure — the viewer falls
/// back to the SVG truth map instead of a dead canvas.)
export function theaterAvailable(): boolean {
  return !theaterFailed;
}

/// Create the Pixi app once, lazily; reused across opens forever after.
/// SINGLE-FLIGHT: concurrent attaches during the first init await the same
/// promise (a second caller must never see a half-initialized app).
function ensureApp(): Promise<void> {
  if (appInit) return appInit;
  appInit = initApp().catch((e) => {
    theaterFailed = true;
    app = null;
    throw e;
  });
  return appInit;
}

async function initApp(): Promise<void> {
  app = new Application();
  holder = document.createElement("div");
  holder.className = "bv-theater-holder";
  tooltip = document.createElement("div");
  tooltip.className = "bv-theater-tip";
  tooltip.style.display = "none";
  await app.init({
    width: CANVAS_W,
    height: CANVAS_H,
    background: "#05070d",
    antialias: true,
    autoDensity: true,
    resolution: Math.min(window.devicePixelRatio || 1, 2),
  });
  holder.appendChild(app.canvas);
  holder.appendChild(tooltip);
  layers = {
    backdrop: new Container(),
    debris: new Container(),
    wrecks: new Container(),
    ships: new Container(),
    fx: new Container(),
    ui: new Container(),
  };
  battleModels = new BattleModels();
  layers.ships.sortableChildren = true; // depth order on the tilted plane
  layers.wrecks.sortableChildren = true;
  app.stage.addChild(layers.backdrop, layers.debris, layers.wrecks, layers.ships, battleModels.sprite, layers.fx, layers.ui);
  debrisG = new Graphics();
  const debrisLayer = new Container();
  layers.debris.addChild(debrisG, debrisLayer);
  fxDebris = new FxSpritePool(debrisLayer, "normal");
  fxTex = createFxTextures();
  const smokeLayer = new Container();
  const lightLayer = new Container();
  layers.fx.addChild(smokeLayer, lightLayer);
  fxSmoke = new FxSpritePool(smokeLayer, "normal");
  fxLight = new FxSpritePool(lightLayer, "add");
  banner = document.createElement("div");
  banner.className = "bv-theater-banner";
  banner.style.display = "none";
  holder.appendChild(banner);
  cameraHint = document.createElement("div");
  cameraHint.className = "bv-theater-hint";
  cameraHint.textContent = "scroll zoom · drag pan · double-click resets";
  holder.appendChild(cameraHint);
  viewToggle = document.createElement("button");
  viewToggle.type = "button";
  viewToggle.className = "bv-theater-view";
  viewToggle.addEventListener("click", () => theaterSetView(!viewAngled));
  holder.appendChild(viewToggle);
  syncViewToggle();
  // Camera controls: wheel zooms toward the cursor, drag pans, double-click
  // returns to auto-fit. (The theater takes no input that isn't a camera or
  // transport control — the standing interaction law.)
  app.canvas.addEventListener("wheel", (ev: WheelEvent) => {
    if (!app) return;
    ev.preventDefault();
    const r = app.canvas.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return; // collapsed layout — no math on it
    const mx = ((ev.clientX - r.left) / r.width) * CANVAS_W;
    const my = ((ev.clientY - r.top) / r.height) * CANVAS_H;
    const wx = ax(mx), wy = ay(my); // arena point under the cursor…
    camZoom = Math.min(CAM_ZOOM_MAX, Math.max(CAM_ZOOM_MIN, camZoom * Math.exp(-ev.deltaY * 0.0016)));
    camX = wx - (mx - CANVAS_W / 2) / (SCALE * camZoom); // …stays under it
    camY = wy - (my - CANVAS_H / 2) / (SCALE * camZoom * tiltY);
    cameraOverview = false;
  }, { passive: false });
  app.canvas.addEventListener("pointerdown", (ev: PointerEvent) => {
    activePointers.set(ev.pointerId, [ev.clientX, ev.clientY]);
    if (activePointers.size === 1) {
      dragging = true;
      dragMoved = false;
      pressAt = [ev.clientX, ev.clientY];
      dragLast = [ev.clientX, ev.clientY];
    } else if (activePointers.size === 2) {
      dragging = false;
      dragLast = null;
      const [a, b] = [...activePointers.values()];
      const r = app!.canvas.getBoundingClientRect();
      const mx = ((((a[0] + b[0]) / 2) - r.left) / r.width) * CANVAS_W;
      const my = ((((a[1] + b[1]) / 2) - r.top) / r.height) * CANVAS_H;
      pinchStart = {
        distance: Math.max(1, Math.hypot(a[0] - b[0], a[1] - b[1])),
        zoom: camZoom,
        worldX: ax(mx),
        worldY: ay(my),
      };
    }
    app?.canvas.setPointerCapture?.(ev.pointerId);
  });
  const endPointer = (ev: PointerEvent) => {
    // A lone press that never became a drag is a click: pick a hull.
    if (ev.type === "pointerup" && activePointers.size === 1 && activePointers.has(ev.pointerId) && !dragMoved) pickShip(ev);
    activePointers.delete(ev.pointerId);
    pinchStart = null;
    const remaining = [...activePointers.values()][0];
    dragging = !!remaining;
    dragLast = remaining ?? null;
  };
  app.canvas.addEventListener("pointerup", endPointer);
  app.canvas.addEventListener("pointercancel", endPointer);
  app.canvas.addEventListener("dblclick", resetCamera);
  // Pointer-move does double duty: drag = pan; hover = torpedo-arc tooltip
  // (arcs are immediate-mode, so the canvas hit-tests the few live arc heads).
  app.canvas.addEventListener("pointermove", (ev: PointerEvent) => {
    if (activePointers.has(ev.pointerId)) activePointers.set(ev.pointerId, [ev.clientX, ev.clientY]);
    if (activePointers.size >= 2 && pinchStart && app) {
      const [a, b] = [...activePointers.values()];
      const r0 = app.canvas.getBoundingClientRect();
      if (r0.width < 2 || r0.height < 2) return;
      const distance = Math.max(1, Math.hypot(a[0] - b[0], a[1] - b[1]));
      const mx = ((((a[0] + b[0]) / 2) - r0.left) / r0.width) * CANVAS_W;
      const my = ((((a[1] + b[1]) / 2) - r0.top) / r0.height) * CANVAS_H;
      camZoom = Math.min(CAM_ZOOM_MAX, Math.max(CAM_ZOOM_MIN, pinchStart.zoom * distance / pinchStart.distance));
      camX = pinchStart.worldX - (mx - CANVAS_W / 2) / (SCALE * camZoom);
      camY = pinchStart.worldY - (my - CANVAS_H / 2) / (SCALE * camZoom * tiltY);
      cameraOverview = false;
      hideTip();
      return;
    }
    if (dragging && dragLast && activePointers.size === 1 && app) {
      if (!dragMoved) {
        // A click's wobble isn't a drag; past it, looking around frees the
        // camera from the followed hull (the selection stays).
        if (Math.hypot(ev.clientX - pressAt[0], ev.clientY - pressAt[1]) < DRAG_START_PX) return;
        dragMoved = true;
        following = false;
        followGlide = null;
        cameraOverview = false;
      }
      const r0 = app.canvas.getBoundingClientRect();
      if (r0.width < 2 || r0.height < 2) return;
      const kx = CANVAS_W / r0.width, ky = CANVAS_H / r0.height;
      camX -= ((ev.clientX - dragLast[0]) * kx) / (SCALE * camZoom);
      camY -= ((ev.clientY - dragLast[1]) * ky) / (SCALE * camZoom * tiltY);
      dragLast = [ev.clientX, ev.clientY];
      hideTip();
      return;
    }
    if (!st?.fx || !tooltip || !app) return;
    const r = app.canvas.getBoundingClientRect();
    const mx = ((ev.clientX - r.left) / r.width) * CANVAS_W;
    const my = ((ev.clientY - r.top) / r.height) * CANVAS_H;
    for (const t of st.fx.torps) {
      const head = torpHead(t, st.frac);
      if (!head || Math.hypot(sx(head[0]) - mx, sy(head[1]) - my) >= 10) continue;
      const target = t.to !== null ? st.ships[t.to] : undefined;
      tooltip.textContent = `Torpedo → ${target ? theaterShipAppearance(st.rec, target.side, target.kind, st.pirateId).label : "target"}`;
      tooltip.style.display = "block";
      tooltip.style.left = `${Math.min(CANVAS_W - 160, Math.max(4, mx + 10))}px`;
      tooltip.style.top = `${Math.max(4, my - 24)}px`;
      return;
    }
    for (const a of st.fx.arcs) {
      if (st.frac > a.tEnd) continue;
      const [hx, hy] = arcHead(a, st.frac);
      if (Math.hypot(sx(hx) - mx, sy(hy) - my) < 10) {
        const target = st.ships[a.to];
        tooltip.textContent = `Torpedo salvo → ${target ? theaterShipAppearance(st.rec, target.side, target.kind, st.pirateId).label : "target"}`;
        tooltip.style.display = "block";
        tooltip.style.left = `${Math.min(CANVAS_W - 160, Math.max(4, mx + 10))}px`;
        tooltip.style.top = `${Math.max(4, my - 24)}px`;
        return;
      }
    }
  });
  app.ticker.add(() => frame(app!.ticker.deltaMS / 1000));
  // Runs ONLY while the viewer shows the theater (theaterClose stops it); a
  // hidden browser tab throttles to nothing on its own (the ticker is
  // rAF-driven), so the map's performance is never affected either way.
  app.ticker.stop();
}

// --- Public API (driven by main.ts's battle-viewer transport) ---------------------

/// Mount the theater into the viewer's placeholder div (re-appended across the
/// viewer's innerHTML rebuilds — the WebGL canvas survives), and (re)bind the
/// record. Safe to call every render.
let attachGen = 0; // bumped by theaterClose — cancels in-flight attaches

export function theaterAttach(
  mount: HTMLElement,
  rec: BattleRecordView,
  pirateId: PlayerId | null = null,
  viewport: TheaterViewport = { width: DEFAULT_CANVAS_W, height: DEFAULT_CANVAS_H },
  scenerySource?: BattleScenerySource,
): void {
  const gen = attachGen;
  const resized = configureViewport(viewport);
  ensureApp().then(
    () => {
      // A close that landed while init was in flight wins: stay closed.
      if (gen !== attachGen || !holder || !app) return;
      if (holder.parentElement !== mount) mount.appendChild(holder);
      bindRecord(rec, pirateId, scenerySource);
      if (resized) buildBackdrop(rec);
      coarsePointer = matchMedia("(pointer: coarse)").matches;
      syncCameraHint();
      // One cap for 2D and 3D (see FAST_FPS). Replay time is still the
      // transport clock. Attaching again (every render) keeps the ladder.
      const preferred = Math.min(FAST_FPS, viewport.maxFps && viewport.maxFps > 0 ? viewport.maxFps : FAST_FPS);
      if (preferred !== preferredFps) {
        preferredFps = targetFps = preferred;
        fpsEma = preferred;
        fastRetryAt = 0;
        fastRetryWait = 20;
      }
      app.ticker.maxFPS = targetFps;
      reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
      app.ticker.start();
    },
    () => {
      // Init failed (no WebGL, context exhausted): the next render falls
      // back to the SVG truth map via theaterAvailable().
    },
  );
}

/// The transport pushes time every animation frame (round + fractional
/// progress through it, whether playing, whether pinned LIGHT-LIVE).
export function theaterSetTime(round: number, frac: number, live: boolean): void {
  if (!st) return;
  st.round = round;
  st.frac = Math.max(0, Math.min(1, frac));
  st.live = live;
}

/// Reliable touch counterpart to double-click. Mobile chrome calls this from
/// an explicit button; keeping the reset in the theater preserves one camera
/// law across mouse and touch without synthesizing a fragile gesture.
export function theaterResetCamera(): void {
  resetCamera();
}

/// The viewer closed — stop rendering entirely (the map is never affected).
export function theaterClose(): void {
  attachGen++; // cancel any attach still awaiting init
  st = null;
  backdropVisual = null;
  backdropPlan = null;
  backdropKey = "";
  activePointers.clear();
  pinchStart = null;
  dragging = false;
  dragLast = null;
  hoverShip = null;
  hintKey = "";
  app?.ticker?.stop(); // ticker exists only once init completed
  battleModels?.close();
  modelFx = new Set();
  modelWrecks = new Set();
  if (holder?.parentElement) holder.parentElement.removeChild(holder);
}

/// Debug: pump ONE frame synchronously (bypasses rAF — headless/hidden panes
/// suspend it) and render. The acceptance rig steps the scene deterministically
/// with this; production playback uses the ticker.
export function theaterStep(dt = 1 / 60): void {
  if (!st || !app) return;
  frame(dt);
  app.renderer.render(app.stage);
}

/// Debug introspection (acceptance rig + dev probing) — read-only.
export function theaterDebug(): Record<string, unknown> | null {
  if (!st || !layers || !app) return null;
  return {
    ships: st.ships.filter((s) => s.inUse).length,
    hulls: st.ships.filter((s) => s.inUse).map((s) => ({
      cid: s.cid, side: s.side, kind: s.kind, hp: s.hp, reps: s.reps,
      visible: s.hull.visible, alpha: s.hull.alpha,
      at: [Math.round(s.root.position.x), Math.round(s.root.position.y)], // canvas px
    })),
    stations: arenaStations.map((s) => ({
      side: s.side, level: s.level, visible: s.sprite.visible, alpha: s.sprite.alpha,
      at: [Math.round(s.sprite.position.x), Math.round(s.sprite.position.y)], width: Math.round(s.sprite.width),
      loaded: s.sprite.texture !== Texture.EMPTY,
    })),
    canvas: [CANVAS_W, CANVAS_H],
    ticker: app.ticker.started,
    round: st.round,
    frac: st.frac,
    beams: st.fx?.beams.length ?? -1,
    beamHits: st.fx?.beams.filter((b) => !b.miss).length ?? -1,
    beamMisses: st.fx?.beams.filter((b) => b.miss).length ?? -1,
    gunfire: st.fx ? [
      ...st.fx.beams.map((b) => ({ from: b.from, to: b.to, t: b.t, miss: b.miss, weapon: "beam" })),
      ...st.fx.tracers.map((b) => ({ from: b.from, to: b.to, t: b.t, miss: b.miss, weapon: "driver" })),
    ] : [],
    tracers: st.fx?.tracers.length ?? -1,
    arcs: st.fx?.arcs.length ?? -1,
    arcFlak: st.fx?.arcs.filter((a) => a.outcome === "flak").length ?? -1,
    torps: st.fx?.torps.map((t) => ({ id: t.id, side: t.side, end: t.end, to: t.to, by: t.by, tEnd: t.tEnd })) ?? [],
    deaths: st.fx?.deaths.map((d) => ({ t: d.t, cls: d.cls, ship: d.shipIdx })) ?? null,
    afterlife: st.rec ? deathTimeline().map((e) => ({ key: e.key, T: +e.T.toFixed(3), breach: +e.breachT.toFixed(3), cls: e.cls, pieces: e.pieces })) : [],
    tier: perfTier,
    fps: { cap: targetFps, preferred: preferredFps, measured: +fpsEma.toFixed(1) },
    follow: { following, cid: followCid, hint: hintKey },
    models: battleModels?.debug() ?? null,
    viewport: { width: CANVAS_W, height: CANVAS_H, scale: +SCALE.toFixed(4) },
    backdrop: backdropVisual ? {
      system: backdropVisual.systemId,
      planets: backdropVisual.planets.length,
      moons: backdropVisual.planets.reduce((n, p) => n + p.moons.length, 0),
      belts: backdropVisual.asteroidBelts.length,
      alpha: backdropSystem?.root.alpha,
      star: backdropSystem?.starLayoutPosition(),
    } : null,
    cam: { x: +camX.toFixed(1), y: +camY.toFixed(1), zoom: +camZoom.toFixed(3), withdrawFrom: st.withdrawFrom },
    view: { angled: viewAngled, tilt: +tiltY.toFixed(3), pitch: +hullPitch().toFixed(4) },
    fxSprites: { light: fxLight?.count ?? 0, smoke: fxSmoke?.count ?? 0 },
  };
}

/// Debug: a stable hash of the CURRENT window's deterministic scene inputs
/// (matched tracks + reps + the window seed). Two independent renders of the
/// same record at the same transport position return the same value.
export function theaterHash(): number {
  if (!st) return 0;
  let h = hashId(`${st.rec.id}:${st.round}`);
  for (const v of st.ships) {
    if (!v.inUse) continue;
    h = (Math.imul(h, 31) + (v.side << 1) + (v.plat ? 1 : 0) + v.kind.charCodeAt(0) * 97) >>> 0;
    h = (Math.imul(h, 31) + Math.round(v.x0 * 10) * 3 + Math.round(v.y0 * 10) * 7) >>> 0;
    h = (Math.imul(h, 31) + Math.round(v.x1 * 10) * 5 + Math.round(v.y1 * 10) * 11) >>> 0;
    h = (Math.imul(h, 31) + Math.round(v.vx0 * 10) * 13 + Math.round(v.vy0 * 10) * 17
      + Math.round(v.vx2 * 10) * 19 + Math.round(v.vy2 * 10) * 23) >>> 0;
    h = (Math.imul(h, 31) + v.reps + Math.round(v.hp * 100) * 13) >>> 0;
  }
  // The FX SCHEDULE is scene truth too — a divergence in any beam, tracer,
  // arc outcome, or death timing must change the hash (the instrument backs
  // the whole determinism law, not just track geometry).
  const fx = st.fx;
  if (fx) {
    for (const b of fx.beams) {
      h = (Math.imul(h, 33) + b.from * 5 + b.to * 13 + Math.round(b.t * 1000) + Math.round(b.w * 100) + (b.heavy ? 17 : 0) + (b.glint ? 29 : 0) + (b.miss ? 43 : 0) + b.missSign) >>> 0;
    }
    for (const tr of fx.tracers) {
      h = (Math.imul(h, 33) + tr.from * 5 + tr.to * 13 + Math.round(tr.t * 1000) + Math.round(tr.strength * 100) + (tr.miss ? 17 : 0) + (tr.spall ? 29 : 0) + tr.missSign) >>> 0;
    }
    for (const a of fx.arcs) {
      h = (Math.imul(h, 33) + a.to * 5 + Math.round(a.x0 * 10) * 3 + Math.round(a.y0 * 10) * 7 + Math.round(a.tEnd * 1000) + (a.outcome === "flak" ? 37 : a.outcome === "hit" ? 41 : 43)) >>> 0;
    }
    for (const d of fx.deaths) {
      h = (Math.imul(h, 33) + Math.round(d.t * 1000) + d.cls * 7 + ((d.shipIdx ?? 63) + 1) * 3) >>> 0;
    }
    for (const t of fx.torps) {
      h = (Math.imul(h, 33) + (t.id % 65521) + ((t.to ?? 63) + 1) * 5 + Math.round(t.tEnd * 1000) + Math.round(t.x1 * 10) * 3
        + TORP_ENDS.indexOf(t.end) * 37) >>> 0;
    }
    for (const r of fx.rams) {
      h = (Math.imul(h, 33) + ((r.ship ?? 63) + 1) * 7 + Math.round(r.t * 1000) + Math.round(r.damage)) >>> 0;
    }
  }
  return h >>> 0;
}

// --- Record binding & backdrop ---------------------------------------------------

/// First round each side runs, from the record's retreat/withdraw beats.
/// `mutual_disengage` (the raid cap + the safety valve — the COMMON endings)
/// carries no side: BOTH sides break off from that round.
function scanWithdraw(rec: BattleRecordView): [number, number] {
  const w: [number, number] = [Infinity, Infinity];
  rec.rounds.forEach((r, i) => {
    for (const n of r.notes) {
      if ((n.kind === "retreat_tripped" || n.kind === "withdraw_ordered") && n.side !== null) {
        w[n.side] = Math.min(w[n.side], i);
      } else if (n.kind === "mutual_disengage") {
        w[0] = Math.min(w[0], i);
        w[1] = Math.min(w[1], i);
      }
    }
  });
  return w;
}

function bindRecord(rec: BattleRecordView, pirateId: PlayerId | null, scenerySource?: BattleScenerySource): void {
  if (st && st.rec.id === rec.id) {
    // Same battle — but the View handler hands us a FRESH record object every
    // ~100 ms (decoded packet identity), so compare CONTENT, not identity. Only real
    // new truth (light extended the rounds, or the outcome/frontier landed)
    // invalidates the active window so tracks, the FX schedule, AND the withdraw
    // flags rebuild. Identity-comparison here rebuilt them all at 10 Hz for the
    // life of an open replay.
    if (
      st.rec.rounds.length !== rec.rounds.length ||
      st.rec.light_frontier_tick !== rec.light_frontier_tick ||
      st.rec.outcome !== rec.outcome
    ) {
      st.windowKey = "";
      st.withdrawFrom = scanWithdraw(rec);
    }
    st.rec = rec;
    st.pirateId = pirateId;
    return;
  }
  st = {
    rec,
    pirateId,
    round: 0,
    frac: 0,
    live: false,
    ships: [],
    fx: null,
    withdrawFrom: [Infinity, Infinity],
    windowKey: "",
  };
  // Withdrawal is rendered from the record's notes: from its first
  // retreat/withdraw beat, a side's ships burn with flared engines while
  // pursuit fire chases them out (the literal pursuit-fire mechanic).
  st.withdrawFrom = scanWithdraw(rec);
  sceneClock = 0;
  bannerKey = "";
  followCid = null; // a new battle follows the player's own hull again
  resetCamera();
  backdropVisual = battleSystemScenery(rec.system, scenerySource);
  backdropPlan = battleBackdropPlan(rec, scenerySource);
  arenaWorldBody = rec.rounds.find((round) => round.frame?.world)?.frame?.world ?? null;
  buildBackdrop(rec);
  buildArenaWorld(rec);
  buildArenaStations(rec);
}

/// §orbital-defense: each side's station, drawn once beneath its hardpoints
/// (which fight as stationary batteries). Its level comes from the record.
let arenaStations: { side: number; level: number; sprite: Sprite; x: number; y: number }[] = [];
/// Across, in arena units, by level (Defense Platform … Star Fortress).
const STATION_SPAN = [70, 90, 130, 180, 320];
/// A hardpoint's turret marker (ship-local px at zoom 1).
const HARDPOINT_PX = 6;

/// Where each side's station stands and where its hardpoints sit on it. The
/// engine lines hardpoints up 20 su apart at the station; the theater spreads
/// them on a ring over the art (by cid), so the turrets read as the station's
/// and fire from it. Hardpoints never move, and they only join a battle at
/// its open, so the first keyframe places them all for the whole record.
interface StationLayout { centers: Map<number, { x: number; y: number; level: number }>; slots: Map<number, [number, number]> }
let layoutId = "";
let layoutCache: StationLayout = { centers: new Map(), slots: new Map() };
function stationLayout(rec: BattleRecordView): StationLayout {
  const frame = rec.rounds.find((r) => r.frame)?.frame;
  if (!frame) return { centers: new Map(), slots: new Map() };
  if (layoutId === rec.id) return layoutCache;
  layoutId = rec.id;
  const centers: StationLayout["centers"] = new Map();
  const slots: StationLayout["slots"] = new Map();
  for (const side of [0, 1]) {
    const mounts = frame.ships.filter((v) => v.side === side && v.station)
      .sort((a, b) => (a.cid ?? 0) - (b.cid ?? 0));
    if (!mounts.length) continue;
    const level = Math.max(...mounts.map((v) => v.station ?? 1));
    const x = mounts.reduce((sum, v) => sum + v.x, 0) / mounts.length;
    const y = mounts.reduce((sum, v) => sum + v.y, 0) / mounts.length;
    centers.set(side, { x, y, level });
    const r = mounts.length > 1 ? STATION_SPAN[level - 1] * 0.26 : 0;
    mounts.forEach((v, k) => {
      const a = -Math.PI / 2 + (2 * Math.PI * k) / mounts.length;
      if (v.cid !== undefined) slots.set(v.cid, [x + Math.cos(a) * r, y + Math.sin(a) * r]);
    });
  }
  layoutCache = { centers, slots };
  return layoutCache;
}

function buildArenaStations(rec: BattleRecordView): void {
  for (const s of arenaStations) s.sprite.destroy();
  arenaStations = [];
  if (!layers) return;
  for (const [side, { x, y, level }] of stationLayout(rec).centers) {
    const sprite = new Sprite(Texture.EMPTY);
    sprite.anchor.set(0.5);
    sprite.visible = false;
    layers.debris.addChild(sprite);
    const entry = { side, level, sprite, x, y };
    arenaStations.push(entry);
    const url = stationArtUrl(level, 512);
    const cached = texCache.get(url);
    if (cached) sprite.texture = cached;
    else if (!texCache.has(url)) {
      void Assets.load<Texture>(url).then((texture) => {
        texture.source.autoGenerateMipmaps = true;
        texCache.set(url, texture);
        if (st?.rec.id === rec.id) for (const s of arenaStations) if (s.level === level) s.sprite.texture = texture;
      }, () => texCache.set(url, null));
    }
  }
}

/// Each station under its hardpoints; silenced (none left firing), it dims.
function drawArenaStations(): void {
  if (!st) return;
  for (const s of arenaStations) {
    const tex = s.sprite.texture;
    s.sprite.visible = tex !== Texture.EMPTY && tex.width > 0;
    if (!s.sprite.visible) continue;
    s.sprite.position.set(sx(s.x), sy(s.y));
    s.sprite.scale.set((STATION_SPAN[s.level - 1] * SCALE * camZoom) / tex.width);
    const firing = st.ships.some((v) => v.inUse && v.side === s.side && v.station > 0 && v.hp > 0);
    s.sprite.alpha = firing ? 1 : 0.45;
  }
}

/// The world a v14 battle is fought at, as a sphere on the battlefield plane:
/// its public art (the system's defended world when known, else plain rock),
/// lit from the star's side. Positioned and sized every frame (drawArenaWorld).
function buildArenaWorld(rec: BattleRecordView): void {
  arenaWorld?.destroy({ children: true });
  arenaWorld = null;
  arenaWorldDisc = null;
  if (!layers || !arenaWorldBody) return;
  const kind = backdropPlan?.world?.kind ?? "barren";
  const art = planetArtwork(kind);
  const world = new Container();
  const disc = new Sprite(Texture.EMPTY);
  disc.anchor.set(art.anchor[0], art.anchor[1]);
  disc.tint = 0xc2cad6;
  world.addChild(disc);
  const shadeTex = terminatorTexture();
  if (shadeTex) {
    const shade = new Sprite(shadeTex);
    shade.anchor.set(0.5);
    shade.rotation = (backdropPlan?.starBearing ?? -2.4) + Math.PI;
    shade.scale.set(1 / shadeTex.width);
    shade.label = "shade";
    world.addChild(shade);
  }
  layers.debris.addChildAt(world, 0);
  arenaWorld = world;
  arenaWorldDisc = disc;
  const url = planetArtUrl(kind, 1254);
  const apply = (texture: Texture) => { disc.texture = texture; };
  const cached = texCache.get(url);
  if (cached) apply(cached);
  else if (!texCache.has(url)) {
    void Assets.load<Texture>(url).then((texture) => {
      texture.source.autoGenerateMipmaps = true;
      texCache.set(url, texture);
      if (st?.rec.id === rec.id && arenaWorldDisc === disc) apply(texture);
    }, () => texCache.set(url, null));
  }
}

/// Keep the arena world under the camera: centred on its arena point, its
/// diameter the world's in arena units at the current zoom.
function drawArenaWorld(): void {
  if (!arenaWorld || !arenaWorldBody || !arenaWorldDisc) return;
  const diameter = 2 * arenaWorldBody.r * SCALE * camZoom;
  arenaWorld.position.set(sx(arenaWorldBody.x), sy(arenaWorldBody.y));
  const tex = arenaWorldDisc.texture;
  if (tex && tex !== Texture.EMPTY && tex.width > 0) {
    const art = planetArtwork(backdropPlan?.world?.kind ?? "barren");
    arenaWorldDisc.scale.set(diameter / (art.visualRatio * tex.width));
  }
  const shade = arenaWorld.getChildByLabel("shade") as Sprite | null;
  if (shade) shade.scale.set((diameter * 1.01) / Math.max(1, shade.texture.width));
}

/// The real System View's static astronomy, captured from served knowledge at
/// open. Resize re-lays out that SAME snapshot; incoming Views/rounds never feed
/// dynamic system state into the replay. This screen-space layer is independent
/// of tactical pan/zoom and sits below every ship, projectile and combat label.
function buildBackdrop(rec: BattleRecordView): void {
  if (!layers) return;
  const key = `${rec.id}`;
  if (backdropKey === key) return;
  backdropKey = key;
  // Reuse one scenery scene across battles/resizes, including its cached art.
  // Detach before clearing the disposable starfield; don't destroy shared art
  // or a scene whose asynchronous texture load is still in flight.
  backdropSystem?.root.removeFromParent();
  layers.backdrop.removeChildren().forEach((c) => c.destroy({ children: true }));
  // Seeded starfield everywhere (deterministic per battle — the standing law).
  const rng = mulberry32(hashId(`${rec.id}:backdrop`));
  const field = new Graphics();
  for (let i = 0; i < 90; i++) {
    const x = rng() * CANVAS_W;
    const y = rng() * CANVAS_H;
    const r = 0.4 + rng() * 0.9;
    field.circle(x, y, r).fill({ color: 0xcfe0ff, alpha: 0.08 + rng() * 0.2 });
  }
  layers.backdrop.addChild(field);
  // (The arena ring lives in drawDebris — it must move with the camera; the
  // starfield + star stay static, reading as far-parallax backdrop.)
  const visual = backdropVisual;
  if (!visual) return; // deep-space battles keep exactly their seeded starfield
  if (backdropPlan?.world) {
    // In a world's orbit: that world up close, lit from its star.
    buildOrbitalBackdrop(rec, backdropPlan);
    return;
  }
  const scenery = backdropSystem ??= new SystemViewScene({ sceneryOnly: true });
  const url = starIconUrl(starTypeFor(visual.systemId));
  scenery.root.visible = true;
  scenery.root.alpha = SYSTEM_BACKDROP_ALPHA;
  scenery.setSystem(visual, texCache.get(url) ?? null);
  scenery.layout(CANVAS_W, CANVAS_H, battleSceneryFrame(CANVAS_W, CANVAS_H));
  layers.backdrop.addChild(scenery.root);
  if (!texCache.has(url)) {
    // No timer polling, and late art cannot repaint a different/closed battle.
    void Assets.load<Texture>(url).then((texture) => {
      texCache.set(url, texture);
      if (st?.rec.id === rec.id && backdropVisual === visual) scenery.setSystem(visual, texture);
    }, () => texCache.set(url, null));
  }
}

/// A fight in a world's orbit: the world fills one corner (its lit limb toward
/// the star, the same pose the System View showed when the fight began), the
/// star burns at the opposite edge. Public astronomy only — never names,
/// infrastructure or lights. Static screen space beneath every hull.
function buildOrbitalBackdrop(rec: BattleRecordView, plan: BattleBackdropPlan): void {
  if (!layers || !plan.world) return;
  const w = CANVAS_W;
  const h = CANVAS_H;
  const short = Math.min(w, h);
  const dir = { x: Math.cos(plan.starBearing), y: Math.sin(plan.starBearing) };
  const reach = 0.5 * Math.hypot(w, h);
  const scene = new Container();
  layers.backdrop.addChild(scene);
  // The star: a broad glare plus its artwork near the edge it lies beyond.
  const starX = w / 2 + dir.x * reach * 0.74;
  const starY = h / 2 + dir.y * reach * 0.74;
  const tint = STAR_TINT[plan.starSlug] ?? 0xffe08a;
  const glow = glowSpriteTexture();
  if (glow) {
    const flare = new Sprite(glow);
    flare.anchor.set(0.5);
    flare.tint = tint;
    flare.alpha = 0.62;
    flare.scale.set((short * 1.5) / glow.width);
    flare.position.set(starX, starY);
    scene.addChild(flare);
  }
  const starType = starTypeFor(plan.systemId);
  const starUrl = starIconUrl(starType);
  const star = new Sprite(texCache.get(starUrl) ?? Texture.EMPTY);
  const [ax, ay] = starAnchor(starType);
  star.anchor.set(ax, ay);
  star.position.set(starX, starY);
  const sizeStar = (texture: Texture) => { star.scale.set((short * 0.2) / Math.max(1, starVisualRatio(starType) * texture.width)); };
  if (texCache.get(starUrl)) sizeStar(texCache.get(starUrl)!);
  scene.addChild(star);
  // A v14 battle carries its world on the battlefield plane (drawArenaWorld):
  // keep only the star here.
  if (arenaWorldBody) return;
  // The world: a vast disc whose sunward limb reaches toward the centre.
  const diameter = Math.max(w, h) * 1.15;
  const art = planetArtwork(plan.world.kind);
  const center = { x: w / 2 - dir.x * (diameter / 2 + short * 0.16), y: h / 2 - dir.y * (diameter / 2 + short * 0.16) };
  const world = new Container();
  world.position.set(center.x, center.y);
  scene.addChild(world);
  const ringBack = new Graphics();
  const ringFront = new Graphics();
  const disc = new Sprite(Texture.EMPTY);
  disc.anchor.set(art.anchor[0], art.anchor[1]);
  disc.rotation = towardStarRotation(plan.starBearing);
  disc.tint = 0xc2cad6; // a touch dim, so hulls and fire stay the subject
  const shadeTex = terminatorTexture();
  const shade = shadeTex ? new Sprite(shadeTex) : null;
  if (shade && shadeTex) {
    shade.anchor.set(0.5);
    shade.rotation = plan.starBearing + Math.PI;
    shade.scale.set(diameter * 1.01 / shadeTex.width);
  }
  if (plan.world.ringed) drawRings(ringBack, ringFront, diameter / 2, plan.world.seed, 1);
  world.addChild(ringBack, disc);
  if (shade) world.addChild(shade);
  world.addChild(ringFront);
  const url = planetArtUrl(plan.world.kind, 1254);
  const apply = (texture: Texture) => {
    disc.texture = texture;
    disc.scale.set(diameter / (art.visualRatio * texture.width));
  };
  const cached = texCache.get(url);
  if (cached) apply(cached);
  else if (!texCache.has(url)) {
    void Assets.load<Texture>(url).then((texture) => {
      texture.source.autoGenerateMipmaps = true;
      texCache.set(url, texture);
      if (st?.rec.id === rec.id && backdropPlan === plan) apply(texture);
    }, () => texCache.set(url, null));
  }
  if (!texCache.has(starUrl)) {
    void Assets.load<Texture>(starUrl).then((texture) => {
      texCache.set(starUrl, texture);
      if (st?.rec.id === rec.id && backdropPlan === plan) { star.texture = texture; sizeStar(texture); }
    }, () => texCache.set(starUrl, null));
  }
}

// --- Ship pool -----------------------------------------------------------------------

function makeShipVis(): ShipVis {
  const root = new Container();
  const glow = new Graphics();
  const body = new Graphics();
  const sprite = new Sprite(Texture.EMPTY);
  sprite.anchor.set(0.5);
  sprite.visible = false;
  const badge = new Text({ text: "", style: new TextStyle({ fontSize: 9, fill: 0xcfe0ff, fontFamily: "system-ui" }) });
  badge.anchor.set(0, 1);
  const plate = new Text({ text: "", style: new TextStyle({ fontSize: 9, fill: 0xffe08a, fontFamily: "system-ui" }) });
  plate.anchor.set(0.5, 0);
  const chrome = new Container();
  chrome.eventMode = "none";
  chrome.addChild(badge, plate);
  const hull = new Container();
  hull.eventMode = "none";
  const hullBar = new Graphics();
  hull.addChild(hullBar);
  const engines = new Container();
  const spin = new Container();
  spin.addChild(glow, engines, body, sprite);
  root.addChild(spin);
  root.eventMode = "static";
  root.cursor = "pointer";
  const v: ShipVis = {
    root, spin, sprite, body, glow, chrome, badge, plate, hull, hullBar, hullOffsetPx: 0,
    engines, flames: [], plumes: [], guns: [], vents: [], halfW: 0, halfL: 0, fxSeed: 0,
    modelPx: 0, modelEligible: false,
    inUse: false, x0: 0, y0: 0, x1: 0, y1: 0, hdg: 0, hdg0: 0, deathT: null,
    vx0: 0, vy0: 0, vx2: 0, vy2: 0, speed: 0, burn: 0, hitTimes: [],
    side: 0, kind: "raider", plat: false, station: 0, hp: 1, hpStart: 1, hpEnd: 1, hpT: 1, reps: 1, seed: 0,
    entered: false, exiting: false,
  };
  // Pointable area: the hull's footprint, never under HOVER_MIN_PX on screen.
  // A ship that is not drawn (dead, not yet arrived) cannot be pointed at.
  root.hitArea = { contains: (x: number, y: number) => {
    if (root.alpha <= 0.01) return false;
    const r = Math.max(v.modelPx * 0.55, HOVER_MIN_PX / Math.max(0.01, root.scale.x));
    return x * x + y * y <= r * r;
  } };
  root.on("pointerover", () => { hoverShip = v; showTip(v); });
  root.on("pointermove", () => { hoverShip = v; showTip(v); });
  root.on("pointerout", () => { if (hoverShip === v) hoverShip = null; hideTip(); });
  return v;
}

function acquireShip(): ShipVis {
  let v = shipPool.find((s) => !s.inUse);
  if (!v) {
    v = makeShipVis();
    shipPool.push(v);
    layers!.ships.addChild(v.root);
    layers!.ui.addChild(v.chrome, v.hull);
  }
  v.inUse = true;
  v.root.visible = true;
  v.hull.visible = true;
  v.chrome.visible = true;
  return v;
}

function releaseAllShips(): void {
  for (const v of shipPool) {
    v.inUse = false;
    v.root.visible = false;
    v.hull.visible = false;
    v.chrome.visible = false;
  }
}

function showTip(v: ShipVis): void {
  if (!tooltip || !st) return;
  const side = st.rec.sides[v.side];
  const fits = (side?.loadouts ?? []).filter((l) => l.kind === v.kind && l.modules.length > 0);
  const fitLine = fits.length ? fits.map((f) => f.modules.join("+")).join(" · ") : "unfitted";
  const label = v.station ? `${stationName(v.station)} hardpoint` : v.plat ? "Defense Platform tier"
    : `${theaterShipAppearance(st.rec, v.side, v.kind, st.pirateId).label} ×${v.reps}`;
  tooltip.textContent = `${label} — ${Math.round(v.hp * 100)}% hull · ${v.plat ? "station" : fitLine}`;
  tooltip.style.display = "block";
  const p = v.root.position;
  tooltip.style.left = `${Math.min(CANVAS_W - 180, Math.max(4, p.x + 10))}px`;
  tooltip.style.top = `${Math.max(4, p.y - 26)}px`;
}
function hideTip(): void {
  if (tooltip) tooltip.style.display = "none";
}

// --- Keyframe matching (the identity layer) -----------------------------------------

type KfShipView = KeyframeView["ships"][number];

const matchFrames = matchBattleFrames;

/// Representative multiplier: how many hulls of (side, kind) each shown
/// sprite stands for this round (count-stack philosophy on screen).
function repsFor(rec: BattleRecordView, round: number, frame: KeyframeView): Map<string, number> {
  const shown = new Map<string, number>();
  for (const s of frame.ships) {
    if (s.plat) continue;
    const k = `${s.side}:${s.kind}`;
    shown.set(k, (shown.get(k) ?? 0) + 1);
  }
  const reps = new Map<string, number>();
  const rd = rec.rounds[round];
  for (const [key, n] of shown) {
    const [sideS, kind] = key.split(":");
    const rc = rd?.counts[Number(sideS)]?.find((c) => c.kind === (kind as ShipKind));
    const total = rc?.exact ?? n;
    reps.set(key, Math.max(1, Math.round(total / n)));
  }
  return reps;
}

// --- Facing ----------------------------------------------------------------------------

/// The engine records positions only; a hull's facing is presentation. At each
/// keyframe a hull turns toward its direction of travel (its track's tangent
/// there) by at most its class's TURN_RATE per round, and a hull barely moving
/// keeps its facing — so capitals come about slowly and never spin while
/// holding station. One chain from the battle's first keyframe, cached per
/// record: a pure function of the arrived rounds, so every window, scrub and
/// replay agrees (only the newest keyframe can settle as later light arrives).
const MOVING = 1.5; // arena units per round below which a hull holds its facing
let facingRec: BattleRecordView | null = null;
let facingRounds = -1;
let facing = new Map<KfShipView, number>();
function facingTable(rec: BattleRecordView): Map<KfShipView, number> {
  if (facingRec === rec && facingRounds === rec.rounds.length) return facing;
  facingRec = rec;
  facingRounds = rec.rounds.length;
  facing = new Map();
  // Consecutive keyframes (a frameless round holds the previous keyframe).
  const frames = rec.rounds.flatMap((r) => (r.frame ? [r.frame.ships] : []));
  const prev = frames.map(() => new Map<KfShipView, KfShipView>());
  const next = frames.map(() => new Map<KfShipView, KfShipView>());
  for (let k = 1; k < frames.length; k++) {
    for (const tr of matchFrames(frames[k - 1], frames[k])) {
      if (tr.from && tr.to) {
        prev[k].set(tr.to as KfShipView, tr.from as KfShipView);
        next[k - 1].set(tr.from as KfShipView, tr.to as KfShipView);
      }
    }
  }
  for (let k = 0; k < frames.length; k++) {
    const cx = [0, 0], cy = [0, 0], cn = [0, 0];
    for (const s of frames[k]) { cx[s.side] += s.x; cy[s.side] += s.y; cn[s.side]++; }
    for (const s of frames[k]) {
      if (s.plat) { facing.set(s, -Math.PI / 2); continue; } // stations stand as drawn
      const before = prev[k].get(s), after = next[k].get(s);
      const a = before ?? s, b = after ?? s;
      const tx = b.x - a.x, ty = b.y - a.y; // central difference (one-sided at the ends)
      const span = (before ? 1 : 0) + (after ? 1 : 0);
      const moving = span > 0 && Math.hypot(tx, ty) > MOVING * span;
      const earlier = before ? facing.get(before) : undefined;
      let heading: number;
      if (earlier !== undefined) {
        heading = moving ? turnToward(earlier, Math.atan2(ty, tx), TURN_RATE[s.kind] ?? 0.6) : earlier;
      } else if (moving) {
        heading = Math.atan2(ty, tx);
      } else {
        // A hull's first sighting at rest faces the other side's centre.
        const e = 1 - s.side;
        heading = cn[e] > 0 ? Math.atan2(cy[e] / cn[e] - s.y, cx[e] / cn[e] - s.x) : s.side === 0 ? 0 : Math.PI;
      }
      facing.set(s, heading);
    }
  }
  return facing;
}

// --- Window (re)build ------------------------------------------------------------------

/// What buildFxWindow reads (and writes) per tracked ship: satisfied by the
/// pooled ShipVis and by the plain tracks the death timeline dry-runs.
type FxShip = Pick<ShipVis, "inUse" | "cid" | "side" | "kind" | "plat" | "station" | "reps" | "x0" | "y0" | "x1" | "y1"
  | "vx0" | "vy0" | "vx2" | "vy2" | "exiting" | "hpStart" | "hpEnd" | "hpT" | "hitTimes" | "hdg0" | "hdg">;

interface WindowPlan {
  wseed: number;
  tracks: { src: KfShipView; entered: boolean; ship: FxShip; seed: number; fxSeed: number }[];
}

/// One window's tracked ships as plain data. rebuildWindow dresses them as
/// sprites and the death timeline dry-runs the FX schedule on them, so both
/// see exactly the same window.
function planWindow(rec: BattleRecordView, round: number): WindowPlan | null {
  // Rounds are 1:1 with engine steps and each carries its step's keyframe.
  // For the rare frameless round (a finalize tail, or a legacy record) HOLD
  // the nearest previous keyframe rather than blanking the battlefield.
  let fi = round;
  let f0 = rec.rounds[fi]?.frame;
  while (!f0 && fi > 0) f0 = rec.rounds[--fi]?.frame;
  // No keyframe anywhere up to this round (e.g. a sensor-estimate record —
  // frames ride participant fidelity only): no ships, no FX.
  if (!f0) return null;
  // The previous window's motion carries each ship's velocity into this one
  // (track curves, smoke trails); facings come from the record-wide chain.
  const fPrev = fi > 0 ? rec.rounds[fi - 1]?.frame : undefined;
  const face = facingTable(rec);
  const prevMove = new Map<KfShipView, [number, number]>();
  if (fPrev) {
    for (const tr of matchFrames(fPrev.ships, f0.ships)) {
      if (tr.from && tr.to) prevMove.set(tr.to, [tr.to.x - tr.from.x, tr.to.y - tr.from.y]);
    }
  }
  const f1 = rec.rounds[round + 1]?.frame ?? f0;
  let tracks = matchFrames(f0.ships, f1.ships);
  // The following window's motion (only once its light has arrived) lets the
  // track curve carry each ship's velocity through the next keyframe.
  const f2 = rec.rounds[round + 2]?.frame;
  const nextMove = new Map<KfShipView, [number, number]>();
  if (f2 && f1 !== f0) {
    for (const tr of matchFrames(f1.ships, f2.ships)) {
      if (tr.from && tr.to) nextMove.set(tr.from, [tr.to.x - tr.from.x, tr.to.y - tr.from.y]);
    }
  }
  // BUDGET: ≤80 ship sprites. A keyframe can carry up to 60/side + capitals;
  // over the cap we keep the heaviest hulls (capitals + platforms always) and
  // the dropped light hulls fold into the survivors' ×N badges via repsFor
  // (count-stack philosophy — nothing vanishes from the accounting).
  const SHIP_SPRITE_CAP = 80;
  if (tracks.length > SHIP_SPRITE_CAP) {
    tracks = tracks
      .map((tr, ix) => ({ tr, ix }))
      .sort((a, b) => {
        const sa = a.tr.from ?? a.tr.to!;
        const sb = b.tr.from ?? b.tr.to!;
        const wa = (sa.plat ? 1e9 : 0) + (MASS[sa.kind] ?? 0);
        const wb = (sb.plat ? 1e9 : 0) + (MASS[sb.kind] ?? 0);
        return wb - wa || a.ix - b.ix; // stable: mass desc, keyframe order
      })
      .slice(0, SHIP_SPRITE_CAP)
      .sort((a, b) => a.ix - b.ix)
      .map((e) => e.tr);
  }
  const shownShips = tracks.map((tr) => (tr.from ?? tr.to!));
  const reps = repsFor(rec, round, { ...f0, ships: shownShips });
  const wseed = hashId(`${rec.id}:${round}`);
  return {
    wseed,
    tracks: tracks.map((tr, i) => {
      const src = (tr.from ?? tr.to!) as KfShipView;
      const plat = src.plat ?? false;
      const hp = Math.max(0, Math.min(1, src.hp));
      const slot = src.station && src.cid !== undefined ? stationLayout(rec).slots.get(src.cid) : undefined;
      const x0 = slot?.[0] ?? src.x, y0 = slot?.[1] ?? src.y;
      const x1 = slot?.[0] ?? (tr.to ?? tr.from!).x, y1 = slot?.[1] ?? (tr.to ?? tr.from!).y;
      const dx = x1 - x0, dy = y1 - y0;
      const move0 = tr.from ? prevMove.get(tr.from as KfShipView) : undefined;
      const move2 = tr.to ? nextMove.get(tr.to as KfShipView) : undefined;
      // Facing at the window's two keyframes (an arrival or departure holds).
      const hdg0 = face.get(src) ?? (src.side === 0 ? 0 : Math.PI);
      return {
        src,
        entered: tr.from === null,
        seed: (wseed ^ Math.imul(i + 1, 0x9e3779b9)) >>> 0,
        fxSeed: hashId(`${rec.id}:hull:${src.cid ?? `${src.side}:${src.kind}:${i}`}`),
        ship: {
          inUse: true, cid: src.cid, side: src.side, kind: src.kind, plat, station: src.station ?? 0,
          reps: plat ? 1 : (reps.get(`${src.side}:${src.kind}`) ?? 1),
          x0, y0, x1, y1,
          vx0: move0?.[0] ?? dx, vy0: move0?.[1] ?? dy,
          vx2: move2?.[0] ?? dx, vy2: move2?.[1] ?? dy,
          exiting: tr.to === null,
          hpStart: hp, hpEnd: Math.max(0, Math.min(1, tr.to?.hp ?? hp)), hpT: 1, hitTimes: [],
          hdg0, hdg: tr.to ? (face.get(tr.to as KfShipView) ?? hdg0) : hdg0,
        },
      };
    }),
  };
}

function rebuildWindow(): void {
  if (!st || !layers) return;
  const { rec, round } = st;
  const key = `${rec.id}:${round}`;
  if (st.windowKey === key) return;
  st.windowKey = key;
  releaseAllShips();
  st.ships = [];
  const plan = planWindow(rec, round);
  if (!plan) {
    st.fx = null;
    debrisG?.clear();
    return;
  }
  for (const t of plan.tracks) {
    const v = acquireShip();
    Object.assign(v, t.ship);
    v.hp = t.ship.hpStart;
    v.entered = t.entered;
    v.seed = t.seed;
    v.fxSeed = t.fxSeed;
    v.speed = 0;
    v.burn = 0;
    v.deathT = null;
    dressShip(v);
    st.ships.push(v);
  }
  st.fx = buildFxWindow(rec, round, st.ships, plan.wseed);
  // Bind each recorded death to its sprite ONCE per window (frame() reads
  // v.deathT — no per-frame index scans).
  for (const d of st.fx.deaths) {
    if (d.shipIdx !== null && st.ships[d.shipIdx]) st.ships[d.shipIdx].deathT = d.t;
  }
}

// --- The FX schedule (all volume derives from the record) --------------------------

/// Served protection fits affect the impact treatment, never whether it hit.
function familyWeights(rec: BattleRecordView, side: number): { pdN: number; reflN: Map<ShipKind, number>; whipN: Map<ShipKind, number> } {
  let pdN = 0;
  const reflN = new Map<ShipKind, number>();
  const whipN = new Map<ShipKind, number>();
  for (const stx of rec.sides[side]?.loadouts ?? []) {
    if (stx.modules.includes("point_defense_screen")) pdN += stx.n;
    if (stx.modules.includes("reflective_plating")) reflN.set(stx.kind, (reflN.get(stx.kind) ?? 0) + stx.n);
    if (stx.modules.includes("whipple_armor")) whipN.set(stx.kind, (whipN.get(stx.kind) ?? 0) + stx.n);
  }
  return { pdN, reflN, whipN };
}

/// Is the `idx`-th shown representative of its (side,kind) group treated as
/// carrying `n` fitted hulls? Deterministic: the FIRST ⌈share⌉ sprites of the
/// group wear the fit (stable order — every viewer sees the same ships glint).
function fittedFlag(ships: FxShip[], v: FxShip, fittedCount: number): boolean {
  if (fittedCount <= 0) return false;
  const group = ships.filter((s) => s.inUse && s.side === v.side && s.kind === v.kind && !s.plat);
  const pos = group.indexOf(v);
  const share = Math.ceil((fittedCount / Math.max(1, group.length * Math.max(1, v.reps))) * group.length);
  return pos >= 0 && pos < Math.max(1, Math.min(group.length, share));
}

function buildFxWindow(rec: BattleRecordView, round: number, ships: FxShip[], wseed: number): FxWindow {
  const fx: FxWindow = { beams: [], tracers: [], arcs: [], pdShips: [], deaths: [], torps: [], rams: [], tackles: [], webs: [] };
  const rng = mulberry32(wseed ^ 0x00f0f0);
  const rd = rec.rounds[round];
  const next = rec.rounds[round + 1];
  const alive = (side: number) => ships.map((v, i) => ({ v, i })).filter((e) => e.v.inUse && e.v.side === side);
  const wA = familyWeights(rec, 0);
  const wD = familyWeights(rec, 1);
  // Frames that record every torpedo replay them exactly; older frames only
  // summarize salvos, and the cosmetic arcs below stand in for them.
  const tracked = !!(rd?.frame?.torps || next?.frame?.torps || next?.frame?.torp_ends);
  if (tracked) {
    fx.torps = torpTracks(rec, round, ships);
    fx.pdShips = pdScreens(rec, ships);
  }
  for (let side = 0 as 0 | 1; !tracked && side < 2; side = (side + 1) as 0 | 1) {
    const foes = alive(1 - side);
    const own = alive(side);
    if (!foes.length || !own.length) continue;
    // Targets weighted by threat MASS (the sim's targeting spirit) — heavies
    // draw fire; the seeded roll keeps every viewer's scene identical.
    const pickTarget = () => {
      const tw = foes.map((f) => Math.pow(MASS[f.v.kind] ?? 800, 0.5));
      let r = rng() * tw.reduce((a, b) => a + b, 0);
      for (let k = 0; k < foes.length; k++) { r -= tw[k]; if (r <= 0) return foes[k].i; }
      return foes[foes.length - 1].i;
    };
    // TORPEDO ARCS — expanded from the salvo summary; the count drop between
    // this frame and the next budgets how many arcs RESOLVE this window, and
    // the defender's PD presence decides how many of those die to flak.
    const s0 = rd?.frame?.torpedoes.find((t) => t.side === side);
    const s1 = next?.frame?.torpedoes.find((t) => t.side === side);
    if (s0 && s0.n > 0) {
      const shown = Math.min(16, s0.n);
      const resolved = Math.round(shown * Math.min(1, Math.max(0, (s0.n - (s1?.n ?? 0)) / s0.n)));
      const foePd = side === 0 ? wD.pdN : wA.pdN;
      const foeShips = (next ?? rd)?.counts[1 - side]?.reduce((a, c) => a + (c.exact ?? 0), 0) ?? 1;
      const pdShare = Math.min(0.85, (foePd / Math.max(1, foeShips)) * 1.6);
      const nFlak = Math.round(resolved * pdShare);
      // PD ships of the defending side screen this window (dense fans).
      const pdKinds = new Set((rec.sides[1 - side]?.loadouts ?? []).filter((l) => l.modules.includes("point_defense_screen")).map((l) => l.kind));
      for (const f of foes) if (pdKinds.has(f.v.kind) || f.v.kind === "dreadnought") fx.pdShips.push(f.i);
      // Arcs sorted so the ones passing nearest PD ships die to flak first.
      const arcs: { d: number; a: FxWindow["arcs"][number] }[] = [];
      for (let k = 0; k < shown; k++) {
        const ox = (rng() - 0.5) * 90, oy = (rng() - 0.5) * 90;
        const to = pickTarget();
        const tv = ships[to];
        const mx = (s0.x + ox + tv.x1) / 2 + (rng() - 0.5) * 260;
        const my = (s0.y + oy + tv.y1) / 2 + (rng() - 0.5) * 260;
        const isResolved = k < resolved;
        const tEnd = isResolved ? 0.45 + rng() * 0.5 : 1.1;
        // Finite base + stable per-arc epsilon: the comparator never sees
        // Infinity−Infinity (NaN comparators are engine-dependent — a replay
        // divergence risk in an effect class the ladder never drops).
        let dPd = 1e9 + k;
        for (const pi of fx.pdShips) {
          const p = ships[pi];
          dPd = Math.min(dPd, Math.hypot(mx - p.x1, my - p.y1));
        }
        // A vanished salvo can also have missed or lost its target. Without
        // hull-loss evidence it fades away; it must not explode on a ship.
        const gunDamage = next?.frame?.gunfire?.filter((s) => s.side === side).reduce((sum, s) => sum + s.damage, 0) ?? 0;
        const damaged = (next?.dealt?.[side] ?? 0) - gunDamage > 1e-4 && tv.hpEnd < tv.hpStart;
        arcs.push({ d: dPd, a: { side, x0: s0.x + ox, y0: s0.y + oy, cx: mx, cy: my, to, tEnd, outcome: isResolved ? (damaged ? "hit" : "fade") : "fly" } });
      }
      arcs.sort((p, q) => p.d - q.d);
      let flakLeft = nFlak;
      for (const e of arcs) {
        if (flakLeft > 0 && e.a.outcome === "fade") { e.a.outcome = "flak"; flakLeft--; }
        fx.arcs.push(e.a);
      }
    }
  }
  // DEATHS — exact record events, timed within the window by their step.
  const deaths = next?.frame?.deaths ?? [];
  if (deaths.length) {
    const steps = deaths.map((d) => d.step);
    const lo = Math.min(...steps), hi = Math.max(...steps);
    for (const d of deaths) {
      const mass = MASS[d.kind] ?? 800;
      const cls = (d.kind === "titan" ? 3 : mass >= 8000 ? 2 : mass >= 2000 ? 1 : 0) as 0 | 1 | 2 | 3;
      // Bind the death to the nearest EXITING sprite of its side+kind so the
      // hull disappears exactly when it dies (not a fade).
      let shipIdx: number | null = null;
      let best = Infinity;
      ships.forEach((v, i) => {
        if (v.inUse && v.exiting && v.side === d.side && v.kind === d.kind && !fx.deaths.some((death) => death.shipIdx === i)) {
          const dist = Math.hypot(v.x0 - d.x, v.y0 - d.y);
          if (dist < best) { best = dist; shipIdx = i; }
        }
      });
      if (shipIdx !== null) {
        const s = ships[shipIdx];
        s.hpEnd = 0;
        // A dying hull is absent from the next keyframe; it flies on to where
        // the engine recorded its death, so the blast is where it is seen to die.
        if (!s.station && Math.hypot(d.x - s.x0, d.y - s.y0) <= (MAX_SPEED[d.kind] ?? 100) * 1.5) {
          s.x1 = d.x; s.y1 = d.y; s.vx2 = d.x - s.x0; s.vy2 = d.y - s.y0;
        }
      }
      // A station hardpoint goes up like a heavy hull, whatever its stand-in kind.
      const hardpoint = shipIdx !== null && (ships[shipIdx] as FxShip).station > 0;
      fx.deaths.push({ t: hi > lo ? 0.15 + 0.7 * ((d.step - lo) / (hi - lo)) : 0.5, x: d.x, y: d.y, kind: d.kind, side: d.side,
        cls: hardpoint ? 1 : cls, shipIdx,
        cid: d.cid ?? (shipIdx !== null ? ships[shipIdx].cid ?? null : null) });
    }
  }
  // Keep one attempt per recorded gunshot, including real misses/cooldowns.
  // Its cosmetic start time is seeded; target, weapon and damage are evidence.
  const gunfire = arrivedGunfire(rec, round, ships);
  gunfire.forEach((shot, k) => {
    const from = ships[shot.from], target = ships[shot.to];
    const foe = from.side === 0 ? wD : wA;
    const strength = Math.min(1, shot.damage / Math.max(1, (MASS[target.kind] ?? 800) * 0.35));
    const t = 0.08 + 0.65 * ((k + 0.2 * rng()) / Math.max(1, gunfire.length));
    const miss = shot.damage <= 0;
    const missSign = rng() < 0.5 ? -1 : 1;
    if (shot.weapon === "beam") {
      fx.beams.push({ t, from: shot.from, to: shot.to, w: 0.65 + 2.6 * strength,
        heavy: from.station > 0 || (MASS[from.kind] ?? 0) >= 8000, miss, missSign,
        glint: !miss && fittedFlag(ships, target, foe.reflN.get(target.kind) ?? 0) });
    } else {
      fx.tracers.push({ t, from: shot.from, to: shot.to, miss, missSign, strength,
        spall: !miss && fittedFlag(ships, target, foe.whipN.get(target.kind) ?? 0) });
    }
  });
  // Rams (rules v9): a hull that ran into wreckage, mid-step.
  for (const r of next?.frame?.rams ?? []) {
    const ship = ships.findIndex((v) => v.inUse && v.cid === r.cid);
    fx.rams.push({ t: 0.4 + 0.2 * jitter(r.cid * 131 + r.hull), ship: ship >= 0 ? ship : null, x: r.x, y: r.y, damage: r.damage });
  }
  // Tackles and webs (rules v11), between hulls this window shows.
  const shownAt = (cid: number) => ships.findIndex((v) => v.inUse && v.cid === cid);
  for (const [links, into] of [[next?.frame?.tackles, fx.tackles], [next?.frame?.webs, fx.webs]] as const) {
    for (const l of links ?? []) {
      const from = shownAt(l.from), to = shownAt(l.to);
      if (from >= 0 && to >= 0) into.push({ from, to });
    }
  }
  // Hull meters and a killing explosion share the last visible impact beat.
  // Both endpoints were already served; this never predicts future hull loss.
  for (let i = 0; i < ships.length; i++) {
    const impacts = [
      ...fx.beams.filter((b) => b.to === i && !b.miss).map((b) => b.t + (b.heavy ? 0 : 0.14 / 1.16)),
      ...fx.tracers.filter((b) => b.to === i && !b.miss).map((b) => b.t + 0.1),
      ...fx.arcs.filter((a) => a.to === i && a.outcome === "hit").map((a) => a.tEnd),
      ...fx.torps.filter((t) => t.to === i && t.end === "hit").map((t) => t.tEnd),
      ...fx.rams.filter((r) => r.ship === i).map((r) => r.t),
    ];
    ships[i].hitTimes = impacts;
    if (impacts.length) {
      ships[i].hpT = Math.max(...impacts);
      const death = fx.deaths.find((d) => d.shipIdx === i);
      if (death) death.t = ships[i].hpT;
    }
  }
  return fx;
}

const TORP_ENDS: TorpTrack["end"][] = ["fly", "launch", "hit", "miss", "intercepted", "dry", "fade", "wreck"];
/// The smoke trail a torpedo drags behind it, arena units (a launch grows into it).
const TORP_TRAIL = 70;
/// How long a missed torpedo coasts on past its target before it gutters out.
const MISS_COAST = 0.3;

/// The window's torpedoes from frames that record each one: those flying on,
/// those that ended (how, where, and when at their real speed), launches (on
/// their launcher until the step ends), and any entering or leaving the
/// per-side cap. Only arrived frames are read; at the light frontier the
/// torpedoes hold where they were last seen.
function torpTracks(rec: BattleRecordView, round: number, ships: FxShip[]): TorpTrack[] {
  const was = rec.rounds[round]?.frame?.torps ?? [];
  const next = rec.rounds[round + 1]?.frame;
  const now = next?.torps;
  const ends = next?.torp_ends ?? [];
  const earlier = new Set((rec.rounds[round - 1]?.frame?.torps ?? []).map((t) => t.id));
  const index = new Map<number, number>();
  ships.forEach((s, i) => { if (s.inUse && s.cid !== undefined) index.set(s.cid, i); });
  const idx = (cid: number | undefined): number | null => (cid === undefined ? null : index.get(cid) ?? null);
  const later = new Map((now ?? []).map((t) => [t.id, t]));
  const ended = new Map(ends.map((e) => [e.id, e]));
  const out: TorpTrack[] = [];
  for (const a of was) {
    const base = { id: a.id, side: a.side, to: idx(a.to), by: null, from: null, x0: a.x, y0: a.y, t0: 0,
      trail0: earlier.has(a.id) ? TORP_TRAIL : 0 };
    const b = later.get(a.id), e = ended.get(a.id);
    if (!now || b) {
      out.push({ ...base, x1: b?.x ?? a.x, y1: b?.y ?? a.y, tEnd: 1, end: "fly" });
    } else if (e) {
      const d = Math.hypot(e.x - a.x, e.y - a.y);
      out.push({ ...base, by: idx(e.by), x1: e.x, y1: e.y, end: e.end,
        tEnd: e.end === "dry" ? 0.3 : Math.min(0.95, Math.max(0.08, d / TORP_SPEED)) });
    } else {
      out.push({ ...base, x1: a.x, y1: a.y, tEnd: 0.4, end: "fade" }); // left the recorded cap
    }
  }
  const seen = new Set(was.map((t) => t.id));
  for (const b of now ?? []) {
    if (seen.has(b.id)) continue;
    // Launched this step: it sits on its launcher, which moved there this window.
    const launcher = ships.findIndex((s) => s.inUse && s.side === b.side && Math.hypot(s.x1 - b.x, s.y1 - b.y) < 2);
    out.push({ id: b.id, side: b.side, to: idx(b.to), by: null, from: launcher >= 0 ? launcher : null,
      x0: b.x, y0: b.y, x1: b.x, y1: b.y, t0: launcher >= 0 ? 0.62 + 0.25 * jitter(b.id) : 0.6, tEnd: 1,
      end: launcher >= 0 ? "launch" : "fly", trail0: 0 });
  }
  for (const e of ends) {
    if (seen.has(e.id)) continue; // over the previous frame's cap: the end alone
    out.push({ id: e.id, side: e.side, to: idx(e.to), by: idx(e.by), from: null, x0: e.x, y0: e.y, x1: e.x, y1: e.y,
      t0: 0.45, tEnd: 0.5, end: e.end, trail0: 0 });
  }
  return out;
}

/// Hulls that project a point-defense bubble: PD-fitted kinds and platforms.
function pdScreens(rec: BattleRecordView, ships: FxShip[]): number[] {
  const fitted = [0, 1].map((side) => new Set((rec.sides[side]?.loadouts ?? [])
    .filter((l) => l.modules.includes("point_defense_screen")).map((l) => l.kind)));
  return ships.flatMap((s, i) => (s.inUse && (s.plat || fitted[s.side]?.has(s.kind)) ? [i] : []));
}

/// Apply art/fallback, scale, team glow, badge, nameplate to a pooled sprite.
function dressShip(v: ShipVis): void {
  if (!st) return;
  const own = st.rec.own_side;
  const mine = own !== null && v.side === own;
  const tint = mine ? TINT_OWN : TINT_FOE;
  // Preload enough native detail for the theater's inspection ceiling. The
  // camera dampens hull magnification by ^0.85; match that AND framebuffer DPI.
  const appearance = theaterShipAppearance(st.rec, v.side, v.kind, st.pirateId,
    spritePx(v.kind) * Math.pow(CAM_ZOOM_MAX, 0.85), app?.renderer.resolution ?? 1);
  // §orbital-defense: a hardpoint is a turret on its station (drawn once, by
  // drawArenaStations), not a station of its own.
  const px = v.station ? HARDPOINT_PX : v.plat ? 26 : spritePx(v.kind) * appearance.calib;
  v.modelPx = px;
  // Hardpoints in ship-local px, shared by the flat art and the 3D hull.
  v.plumes = v.plat ? [] : enginePlumes(v.kind, px);
  v.guns = v.plat ? [[0, 0]] : gunPoints(v.kind, px);
  [v.halfW, v.halfL] = v.plat ? [px * 0.4, px * 0.4] : hullHalfExtents(v.kind, px);
  v.vents = damageVents(v.fxSeed, v.kind, px, v.plat);
  ensureFlames(v);
  // Pirate privateers and defense platforms have their own distinct art, not
  // one of the eighteen approved corporate hull models.
  v.modelEligible = !v.plat && !(v.kind === "raider" && st.pirateId !== null && st.rec.sides[v.side]?.corp === st.pirateId);
  v.sprite.alpha = v.body.alpha = 1;
  const tex = v.station ? null : v.plat
    ? resolveTexture(STATION_ART)
    : resolveTexture(appearance.url);
  if (tex) {
    v.sprite.texture = tex;
    v.sprite.anchor.set(v.plat ? 0.5 : appearance.anchor?.[0] ?? 0.5,
      v.plat ? 0.5 : appearance.anchor?.[1] ?? 0.5);
    v.sprite.visible = true;
    const ratio = tex.height > 0 ? tex.height / tex.width : 1;
    v.sprite.width = px;
    v.sprite.height = px * ratio;
    v.sprite.tint = 0xffffff;
    v.body.clear();
  } else if (v.station) {
    // A turret on the station art: a dark mount, rimmed in its side's colour,
    // with its battery's light.
    v.sprite.visible = false;
    v.body.clear();
    v.body.circle(0, 0, px * 0.5).fill({ color: 0x1b212b, alpha: 0.92 }).stroke({ color: tint, width: 0.7, alpha: 0.9 });
    v.body.circle(0, 0, px * 0.2).fill({ color: 0x8fe9ff, alpha: 0.95 });
  } else {
    // Procedural silhouette, mass-scaled — the fallback idiom.
    v.sprite.visible = false;
    v.body.clear();
    v.body.poly([0, -px * 0.55, px * 0.34, px * 0.45, 0, px * 0.28, -px * 0.34, px * 0.45])
      .fill({ color: 0xc9d6e8, alpha: 0.92 })
      .stroke({ color: tint, width: 1, alpha: 0.9 });
  }
  // Team identity: a faint rim glow in the side colour (art stays natural);
  // the engines burn in the same colour (updateEngines).
  v.glow.clear();
  v.glow.circle(0, 0, px * 0.5).fill({ color: tint, alpha: 0.06 });
  // ×N representative badge (count-stack philosophy on screen).
  v.badge.text = v.reps > 1 ? `×${v.reps}` : "";
  v.badge.position.set(px * 0.4, -px * 0.35);
  // Titan nameplate (the flagship name lives on the syndicate, as shipped).
  const name = !v.plat && v.kind === "titan" ? st.rec.sides[v.side]?.flagship_name ?? "" : "";
  v.plate.text = name;
  v.plate.position.set(0, px * 0.6);
  v.hullOffsetPx = Math.max(px, v.sprite.visible ? v.sprite.height : px) * 0.6 + (name ? 12 : 0);
  drawHullMeter(v);
}

function drawHullMeter(v: ShipVis): void {
  // Each meter is this representative's recorded hull fraction, never the
  // side's survivor count or a fleet-wide average. Only arrived keyframes feed
  // v.hp; recorded damage steps on its visible impact, never by extrapolation.
  // A bar only (no percentage text): a dark backing keeps it legible over fire.
  const hullColor = v.hp > 0.5 ? 0x8fe3a0 : v.hp > 0.25 ? 0xffce70 : 0xff7882;
  v.hullBar.clear()
    .roundRect(-30, -2, 60, 8, 3).fill({ color: 0x070b15, alpha: 0.9 })
    .rect(-28, 0, 56, 4).fill({ color: 0x354153 });
  if (v.hp > 0) v.hullBar.rect(-28, 0, 56 * v.hp, 4).fill({ color: hullColor });
}

// --- The frame loop --------------------------------------------------------------------

/// The degradation ladder, once a frame (rolling fps estimate). A stage that
/// can't hold its cap above 30 drops to 30 before any effect is cut; at 30 with
/// every effect on, it tries the preferred cap again after `fastRetryWait`.
function pace(dt: number): void {
  fpsEma = fpsEma * 0.95 + (1 / Math.max(1e-3, dt)) * 0.05;
  if (targetFps > SLOW_FPS && fpsEma < targetFps * .85) {
    targetFps = fpsEma = SLOW_FPS;
    fastRetryAt = sceneClock + fastRetryWait;
    fastRetryWait = Math.min(300, fastRetryWait * 2);
    if (app) app.ticker.maxFPS = targetFps;
    return;
  }
  perfTier = fpsEma >= targetFps * .85 ? Math.max(0, perfTier - (fpsEma > targetFps * .92 ? 1 : 0)) : Math.min(3, perfTier + 1);
  if (targetFps < preferredFps && perfTier === 0 && fastRetryAt > 0 && sceneClock >= fastRetryAt) {
    targetFps = fpsEma = preferredFps;
    fastRetryAt = 0;
    if (app) app.ticker.maxFPS = targetFps;
  }
}

function frame(dt: number): void {
  if (!st || !layers) return;
  sceneClock += dt;
  if (dt > 0 && sceneClock > PACE_WARMUP) pace(dt);
  // The view angle eases toward the player's choice (a camera control: the
  // replay clock and every recorded fact are untouched).
  const targetTilt = viewAngled ? THEATER_TILT : 1;
  if (tiltY !== targetTilt) {
    tiltY = reducedMotion ? targetTilt : tiltY + (targetTilt - tiltY) * (1 - Math.exp(-Math.max(0, dt) * 9));
    if (Math.abs(targetTilt - tiltY) < 1e-3) tiltY = targetTilt;
  }
  rebuildWindow();
  const { frac, live } = st;
  const atFrontier = st.round >= st.rec.rounds.length - 1;
  const holdLive = live && atFrontier;
  // Camera: the zoom is the player's (wheel/pinch/double-click); the centre
  // follows a hull until a drag frees it. §arena discipline keeps the fight
  // inside the default arena framing; a degenerate camera state self-heals
  // to the overview.
  if (!isFinite(camX) || !isFinite(camY) || !isFinite(camZoom)) resetCamera();
  if (cameraOverview) {
    camX = 0;
    camY = 0;
    camZoom = overviewZoom();
  }
  if (following) followCamera(dt, frac);
  syncCameraHint();
  const zs = Math.pow(camZoom, 0.85); // ships grow with zoom (slightly damped)
  for (const v of st.ships) {
    if (!v.inUse) continue;
    const hp = frac >= v.hpT ? v.hpEnd : v.hpStart;
    if (hp !== v.hp) { v.hp = hp; drawHullMeter(v); }
    // The curved track through the recorded keyframes (trackMotion).
    const m = trackMotion(v, frac);
    const x = m.x, y = m.y, vx = m.vx, vy = m.vy;
    const speed = Math.hypot(vx, vy);
    v.speed = speed;
    v.burn = speed > 1e-6 ? Math.max(0, (m.ax * vx + m.ay * vy) / speed) : 0;
    v.root.position.set(sx(x), sy(y));
    v.root.scale.set(zs, zs * tiltY); // flat art lies on the (tilted) plane
    v.root.zIndex = y; // nearer hulls (lower on screen) draw over farther ones
    // LIGHT-LIVE hold: the newest keyframe breathes — engine flicker + slight
    // station-keeping (a few SCREEN pixels, so it reads at any zoom) — rather
    // than freezing while waiting for light. Wall-clock is allowed here only:
    // the hold is a live view, not a replay (replays never enter this branch).
    if (holdLive) {
      const p1 = jitter(v.seed) * Math.PI * 2;
      const p2 = jitter(v.seed ^ 0x9e37) * Math.PI * 2;
      v.root.position.x += Math.sin(sceneClock * 0.7 + p1) * 3;
      v.root.position.y += Math.cos(sceneClock * 0.55 + p2) * 3;
      v.glow.alpha = 0.8 + 0.2 * Math.sin(sceneClock * 5 + p1);
    } else {
      v.glow.alpha = 1;
    }
    // Rotation is a PURE function of (record, round, frac): the hull turns
    // at a steady rate between the facings of the window's two keyframes
    // (facingTable caps that turn by hull class) — deterministic across
    // renders, scrubs, and frame rates.
    v.spin.rotation = v.hdg0 + wrapAngle(v.hdg - v.hdg0) * frac + Math.PI / 2; // art is nose-up
    // Labels stay horizontal and constant-size whatever the hull/camera do.
    v.badge.rotation = -v.spin.rotation;
    v.plate.rotation = -v.spin.rotation;
    updateEngines(v);
    v.badge.scale.set(1 / zs);
    v.plate.scale.set(1 / zs);
    // Wave arrivals fade in; a ship bound to a recorded DEATH disappears at
    // its exact moment (the explosion takes over); other departures (withdrew
    // or sampled out) fade.
    let a = v.entered ? Math.min(1, frac * 3) : 1;
    if (v.deathT !== null) a = frac >= v.deathT ? 0 : 1;
    else if (v.exiting) a = Math.max(0.15, 1 - frac);
    // Runners DISSOLVE across the withdraw margin (the sim removes them at
    // the exit radius) — nothing slides endlessly off the map.
    if (v.exiting || st.round >= st.withdrawFrom[v.side]) {
      const r = Math.hypot(x, y);
      a *= Math.max(0, Math.min(1, (1400 - r) / 350));
    }
    v.root.alpha = a * (0.45 + 0.55 * v.hp);
    v.chrome.position.copyFrom(v.root.position);
    v.chrome.scale.set(zs);
    v.chrome.rotation = v.spin.rotation;
    v.chrome.alpha = v.root.alpha;
    // UI layer keeps hull meters horizontal and readable at every camera zoom.
    // Only the ship under the viewer's pointer shows its meter (full hulls
    // included), just below the hull and clear of the tooltip above it; it
    // follows the same arrival/death/withdraw visibility as its ship.
    const hullOffset = v.hullOffsetPx * zs + 7;
    v.hull.position.set(v.root.position.x, v.root.position.y + hullOffset);
    v.hull.alpha = a;
    v.hull.visible = a > 0 && v === hoverShip;
  }
  // Keep the ordinary art until that exact hull has rendered successfully.
  // Returning to a budget fallback or losing the context restores it at once.
  placeWreckModels(zs);
  const sample = battleModels?.render(battleModelFrame(), performance.now());
  modelFx = sample?.bursts ?? new Set();
  modelWrecks = sample?.wrecks ?? new Set();
  drawWreckSlices(zs);
  for (let i = 0; i < st.ships.length; i++) {
    const v = st.ships[i], covered = sample?.ships.has(i) ?? false;
    v.sprite.alpha = v.body.alpha = covered ? 0 : 1;
  }
  drawDebris();
  drawArenaWorld();
  drawArenaStations();
  drawFx(dt);
  applyShakeAndBanner();
}

// --- Afterlife: what happens to a hull after its recorded death ------------------

/// One recorded death, placed where and when the hull is SEEN to die: the
/// moment its last evidenced hit lands, on its track, with the velocity and
/// facing it had then. Everything after a death (the breach, sections,
/// wrecks, debris, smoke) is drawn from these as a function of replay time.
interface DeathEvent {
  key: string; // `${window}:${index}` — stable per record
  window: number; // the replay window the death falls in
  T: number; // the mortal blow (absolute replay time, rounds)
  breachT: number; // when the hull breaks apart (capitals burn first)
  cls: 0 | 1 | 2 | 3;
  x: number; y: number; vx: number; vy: number; heading: number; // arena, at T
  kind: ShipKind; side: number; plat: boolean; mass: number; seed: number;
  px: number; // the hull's art size (ship-local px at zoom 1)
  artUrl: string | null; anchor: [number, number]; // its flat art, for slices
  model: boolean; // the hull has an approved 3D model (break-up in 3D)
  pieces: number; cuts: number[]; motions: PieceMotion[];
  yawDrift: number; // the dying hull's slow turn, rad per round
  fadeAt: number; // a capital wreck past MAX_WRECKS fades from here (rounds)
  frags: Fragment[];
  cid: number | null; // the dead hull's battle-local id (its wreckage in rules v9)
}

const HULL_DRAG = 0.6; // a dying hull loses its way over a few rounds
const DEBRIS_DRAG = 0.45;
const BLAST_LIFE = [0.34, 0.44, 0.56, 0.68];
const BLAST_RADIUS = [12, 19, 30, 46]; // px at zoom 1 (3D bursts)

let timelineRec: BattleRecordView | null = null;
let timelineId = "";
let timelineTicks: number[] = [];
let timeline: DeathEvent[] = [];

/// Every arrived death of the open record, in replay order. Each death's
/// window is dry-run through the same plan and FX schedule the live window
/// uses, so the event is exactly where the hull is seen to die. Records only
/// grow (arrived light is a prefix), so as rounds arrive only the newest few
/// windows — whose tracks and facings can still change — are recomputed.
function deathTimeline(): DeathEvent[] {
  const s = st!;
  const rec = s.rec;
  if (timelineRec === rec && timelineTicks.length === rec.rounds.length) return timeline;
  const grown = timelineId === rec.id && rec.rounds.length >= timelineTicks.length
    && timelineTicks.every((tick, k) => rec.rounds[k]?.tick === tick);
  const from = grown ? Math.max(0, timelineTicks.length - 4) : 0;
  timelineRec = rec;
  timelineId = rec.id;
  timelineTicks = rec.rounds.map((r) => r.tick);
  const events: DeathEvent[] = grown ? timeline.filter((e) => e.window < from) : [];
  for (let r = from; r + 1 < rec.rounds.length; r++) {
    if (!rec.rounds[r + 1].frame?.deaths.length) continue;
    const plan = planWindow(rec, r);
    if (!plan) continue;
    const ships = plan.tracks.map((t) => t.ship);
    const fx = buildFxWindow(rec, r, ships, plan.wseed);
    fx.deaths.forEach((d, j) => {
      const ship = d.shipIdx !== null ? ships[d.shipIdx] : null;
      const m = ship ? trackMotion(ship, d.t) : null;
      const plat = ship?.plat ?? false;
      const mass = MASS[d.kind] ?? 800;
      const appearance = theaterShipAppearance(rec, d.side, d.kind, s.pirateId,
        spritePx(d.kind) * Math.pow(CAM_ZOOM_MAX, 0.85), app?.renderer.resolution ?? 1);
      const seed = hashId(`${rec.id}:death:${r}:${j}`);
      const pieces = plat ? 0 : (WRECK_PIECES[d.kind] ?? 0);
      const cuts = wreckCuts(d.kind);
      const cls = ship?.station ? 1 : blastClass(d.kind, mass);
      // Rules v9 record a large hull's wreckage: it breaks when the engine
      // breaks it (a titan a step later), so the blast meets the sections.
      const track = cls >= 2 && d.cid !== null ? wreckTracks().get(d.cid) : undefined;
      events.push({
        key: `${r}:${j}`, window: r, T: r + d.t, cls, cid: d.cid,
        breachT: track ? Math.max(r + d.t + 0.05, track.from + (d.kind === "titan" ? 1 : 0)) : r + d.t + breachDelay(cls),
        x: m?.x ?? d.x, y: m?.y ?? d.y, vx: m?.vx ?? 0, vy: m?.vy ?? 0,
        heading: ship ? ship.hdg0 + wrapAngle(ship.hdg - ship.hdg0) * d.t : d.side === 0 ? 0 : Math.PI,
        kind: d.kind, side: d.side, plat, mass, seed,
        // A station hardpoint is a turret: it bursts, leaving its station.
        px: ship?.station ? HARDPOINT_PX : plat ? 26 : spritePx(d.kind) * appearance.calib,
        artUrl: ship?.station ? null : plat ? STATION_ART : appearance.url,
        anchor: [appearance.anchor?.[0] ?? 0.5, appearance.anchor?.[1] ?? 0.5],
        model: !plat && !(d.kind === "raider" && s.pirateId !== null && rec.sides[d.side]?.corp === s.pirateId),
        pieces, cuts, motions: Array.from({ length: pieces }, (_, i) => pieceMotion(seed, i, pieces, cuts, cls)),
        yawDrift: track ? 0 : (jitter(seed ^ 0x7a11) - 0.5) * (cls >= 2 ? 0.12 : 0.8),
        fadeAt: Infinity,
        frags: fragments(seed, mass),
      });
    });
  }
  events.sort((a, b) => a.T - b.T || a.key.localeCompare(b.key));
  // Capital wrecks stay for the battle; past MAX_WRECKS the oldest fade out
  // as each newer one breaks.
  const wrecks = events.filter((e) => e.cls >= 2 && e.pieces > 0).sort((a, b) => a.breachT - b.breachT);
  for (const w of wrecks) w.fadeAt = Infinity;
  for (let i = 0; i + MAX_WRECKS < wrecks.length; i++) wrecks[i].fadeAt = wrecks[i + MAX_WRECKS].breachT;
  timeline = events;
  return timeline;
}

/// Where a dead hull's wreck is and how it is turned, `s` rounds after the blow.
/// Recorded wreckage (rules v9) by dead hull: from the round it first appears,
/// each round's section positions (arena), by section from the bow.
interface WreckTrack { from: number; pos: ([number, number] | undefined)[][] }
let wreckTrackRec: BattleRecordView | null = null;
let wreckTrackLen = -1;
let wreckTrackIdx = new Map<number, WreckTrack>();
function wreckTracks(): Map<number, WreckTrack> {
  const rec = st!.rec;
  if (wreckTrackRec === rec && wreckTrackLen === rec.rounds.length) return wreckTrackIdx;
  wreckTrackRec = rec;
  wreckTrackLen = rec.rounds.length;
  const idx = new Map<number, WreckTrack>();
  rec.rounds.forEach((round, r) => {
    for (const w of round.frame?.wrecks ?? []) {
      let t = idx.get(w.hull);
      if (!t) { t = { from: r, pos: [] }; idx.set(w.hull, t); }
      (t.pos[r - t.from] ??= [])[w.piece] = [w.x, w.y];
    }
  });
  wreckTrackIdx = idx;
  return idx;
}

/// A large hull's recorded wreckage, if the engine recorded it (rules v9).
function engineWreck(e: DeathEvent): WreckTrack | null {
  return e.cls >= 2 && e.cid !== null ? wreckTracks().get(e.cid) ?? null : null;
}

/// The mean of a recorded wreck's sections in its `j`-th recorded round.
function wreckRoundCentre(t: WreckTrack, j: number): [number, number] | null {
  const row = t.pos[j]?.filter((p): p is [number, number] => !!p) ?? [];
  if (!row.length) return null;
  return [row.reduce((a, p) => a + p[0], 0) / row.length, row.reduce((a, p) => a + p[1], 0) / row.length];
}

/// Where recorded section `i` is `k` rounds after the wreck was first recorded:
/// a straight line between recorded rounds; held after its last record; and
/// before its first, the whole hull running back along its first step.
function wreckPiecePos(t: WreckTrack, i: number, k: number): [number, number] | null {
  const at = (j: number) => t.pos[j]?.[i] ?? null;
  if (k < 0) {
    const p0 = at(0), c0 = wreckRoundCentre(t, 0), c1 = wreckRoundCentre(t, 1);
    if (!p0 || !c0) return null;
    return c1 ? [p0[0] + (c1[0] - c0[0]) * k, p0[1] + (c1[1] - c0[1]) * k] : p0;
  }
  const j = Math.floor(k), u = k - j;
  const a = at(j);
  if (!a) {
    for (let back = Math.min(j, t.pos.length - 1); back >= 0; back--) { const p = at(back); if (p) return p; }
    return null;
  }
  const b = at(j + 1);
  return b ? [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u] : a;
}

function wreckCentreAt(t: WreckTrack, pieces: number, k: number): [number, number] | null {
  let x = 0, y = 0, n = 0;
  for (let i = 0; i < pieces; i++) {
    const p = wreckPiecePos(t, i, k);
    if (p) { x += p[0]; y += p[1]; n++; }
  }
  return n ? [x / n, y / n] : null;
}

/// A recorded wreck's heading: from the dying hull's drawn heading to the line
/// the engine laid its sections along (bow first), settled by the breach.
function wreckHeadingAt(e: DeathEvent, t: WreckTrack, s: number): number {
  const bow = t.pos[0]?.[0], stern = t.pos[0]?.[e.pieces - 1];
  if (!bow || !stern) return e.heading;
  const axis = Math.atan2(bow[1] - stern[1], bow[0] - stern[0]);
  return e.heading + wrapAngle(axis - e.heading) * smooth01(s / Math.max(0.3, e.breachT - e.T));
}

const wreckAt = { x: 0, y: 0, heading: 0 };
function wreckFrame(e: DeathEvent, s: number): typeof wreckAt {
  const track = engineWreck(e);
  const centre = track ? wreckCentreAt(track, e.pieces, e.T + s - track.from) : null;
  if (track && centre) {
    wreckAt.x = centre[0];
    wreckAt.y = centre[1];
    wreckAt.heading = wreckHeadingAt(e, track, s);
    return wreckAt;
  }
  const g = drift(s, HULL_DRAG);
  wreckAt.x = e.x + e.vx * g;
  wreckAt.y = e.y + e.vy * g;
  wreckAt.heading = e.heading + e.yawDrift * drift(s, 0.3);
  return wreckAt;
}

/// A wreck section's pose `b` rounds after the breach (b ≤ 0: still whole).
/// Recorded wreckage (rules v9) sits exactly where the engine has it: each
/// section's offset is its recorded displacement from the hull it was part of.
function pieceState(e: DeathEvent, i: number, s: number, b: number) {
  const m = e.motions[i];
  const u = b > 0 ? m.speed * drift(b, 0.5) : 0;
  const t = b > 0 ? drift(b, 0.15) : 0;
  const heavy = e.cls >= 2;
  const alpha = heavy
    ? 1 - clamp01((st!.round + st!.frac - e.fadeAt) / 3)
    : 1 - smooth01((b - 3) / 3);
  let ox = m.dx * u, oz = m.dz * u;
  const track = engineWreck(e);
  if (track) {
    const k = e.T + s - track.from;
    const p = wreckPiecePos(track, i, k), c = wreckCentreAt(track, e.pieces, k);
    const p0 = wreckPiecePos(track, i, 0), c0 = wreckCentreAt(track, e.pieces, 0);
    if (p && c && p0 && c0) {
      const scale = (e.px / 2.24) * Math.pow(camZoom, 0.85) / (SCALE * camZoom); // arena units per model unit
      const r = wreckHeadingAt(e, track, s) + Math.PI / 2, co = Math.cos(r), sn = Math.sin(r);
      const dx = p[0] - c[0] - (p0[0] - c0[0]), dy = p[1] - c[1] - (p0[1] - c0[1]);
      ox = (dx * co + dy * sn) / scale;
      oz = (-dx * sn + dy * co) / scale;
    }
  }
  return { ox, oz, yaw: m.yaw * t, roll: m.roll * t, pitch: m.pitch * t,
    char: charOf(s, e.breachT - e.T), alpha };
}
const smooth01 = (v: number) => { const c = clamp01(v); return c * c * (3 - 2 * c); };

/// Shock fronts of every blast up to now, so later blasts push older debris.
function pushed(x: number, y: number, born: number, now: number, events: DeathEvent[], out: { x: number; y: number }): void {
  out.x = x; out.y = y;
  for (const e of events) {
    if (e.T > now) break;
    if (e.breachT <= born || e.breachT > now) continue;
    const c = wreckFrame(e, e.breachT - e.T);
    const dx = x - c.x, dy = y - c.y, d = Math.hypot(dx, dy);
    if (d < 1e-6) continue;
    const p = shockPush(e.cls, now - e.breachT, d);
    if (p > 0) { out.x += (dx / d) * p; out.y += (dy / d) * p; }
  }
}

/// The 2D flat-art slices of each hull type (cut where the 3D chunks are).
const sliceCache = new Map<string, Texture[]>();
function artSlices(e: DeathEvent): Texture[] | null {
  if (!e.artUrl || e.pieces < 2) return null;
  const key = `${e.artUrl}|${e.pieces}|${e.kind}`;
  const cached = sliceCache.get(key);
  if (cached) return cached;
  const tex = resolveTexture(e.artUrl);
  if (!tex) return null;
  const f = tex.frame, bounds = [0, ...e.cuts, 1];
  const slices = bounds.slice(0, -1).map((lo, i) => new Texture({ source: tex.source,
    frame: new Rectangle(f.x, f.y + f.height * lo, f.width, Math.max(1, f.height * (bounds[i + 1] - lo))) }));
  sliceCache.set(key, slices);
  return slices;
}

/// Debris cut from each hull's own art, built once per art and hull type when
/// that art has loaded (the painted plates stand in until then, or for good
/// where no canvas can read the art).
const debrisAtlases = new Map<string, DebrisAtlas | null>();
function debrisArt(e: DeathEvent): DebrisAtlas | null {
  if (!e.artUrl) return null;
  const key = `${e.artUrl}|${e.kind}`;
  const hit = debrisAtlases.get(key);
  if (hit !== undefined) return hit;
  const tex = resolveTexture(e.artUrl);
  if (!tex) return null;
  const image = tex.source.resource as CanvasImageSource | undefined;
  const res = tex.source.resolution || 1, f = tex.frame;
  const [x, y, w, h] = [f.x * res, f.y * res, f.width * res, f.height * res];
  const solid = image ? artOpacity(image, x, y, w, h) : null;
  const atlas = image && solid
    ? paintDebrisAtlas(image, x, y, w, h, debrisCuts(hashId(key), solid, h / w, debrisCutFraction(e.mass, e.px)))
    : null;
  debrisAtlases.set(key, atlas);
  return atlas;
}

interface WreckVis { root: Container; spin: Container; sprite: Sprite }
const wreckPool: WreckVis[] = [];
let wreckUsed = 0;
function wreckSprite(): WreckVis | null {
  if (!layers) return null;
  let w = wreckPool[wreckUsed];
  if (!w) {
    const root = new Container(), spin = new Container(), sprite = new Sprite(Texture.EMPTY);
    sprite.anchor.set(0.5);
    root.eventMode = "none";
    spin.addChild(sprite);
    root.addChild(spin);
    layers.wrecks.addChild(root);
    w = { root, spin, sprite };
    wreckPool.push(w);
  }
  wreckUsed++;
  w.root.visible = true;
  return w;
}

/// Wreck sections for the 3D renderer: placed in frame() before the 3D
/// render; flat-art slices fill in afterwards for any section it did not show.
let wreckPieces: BattleWreckPiece[] = [];

/// Visit every visible wreck section this frame: (event, section, pose, screen
/// position of the hull, its heading).
function eachWreckPiece(zs: number, visit: (e: DeathEvent, i: number, p: ReturnType<typeof pieceState>,
  hx: number, hy: number, heading: number, depth: number) => void): void {
  if (!st) return;
  const now = st.round + st.frac;
  for (const e of deathTimeline()) {
    if (e.T > now) break;
    if (e.pieces < 2) continue;
    const s = now - e.T, b = now - e.breachT;
    const hull = wreckFrame(e, s);
    const hx = sx(hull.x), hy = sy(hull.y), heading = hull.heading, depth = hull.y;
    if (hx < -400 || hy < -400 || hx > CANVAS_W + 400 || hy > CANVAS_H + 400) continue;
    void zs;
    for (let i = 0; i < e.pieces; i++) {
      const p = pieceState(e, i, s, b);
      if (p.alpha > 0.01) visit(e, i, p, hx, hy, heading, depth);
    }
  }
}

/// The 3D renderer's wreck sections (only hulls with an approved model).
function placeWreckModels(zs: number): void {
  wreckPieces = [];
  eachWreckPiece(zs, (e, i, p, hx, hy, heading) => {
    if (!e.model) return;
    wreckPieces.push({ id: `${e.key}:${i}`, kind: e.kind, chunk: i, x: hx, y: hy, angle: screenHeading(heading),
      canvasPx: e.px * zs, alpha: p.alpha, char: p.char, offset: [p.ox, 0, p.oz], tumble: [p.pitch, p.yaw, p.roll] });
  });
}

/// Flat-art slices for every section the 3D renderer did not show.
function drawWreckSlices(zs: number): void {
  wreckUsed = 0;
  eachWreckPiece(zs, (e, i, p, hx, hy, heading, depth) => {
    if (modelWrecks.has(`${e.key}:${i}`)) return;
    const slices = artSlices(e);
    const w = slices ? wreckSprite() : null;
    if (!slices || !w) return;
    const tex = slices[i];
    const bounds = [0, ...e.cuts, 1];
    const span = bounds[i + 1] - bounds[i];
    const k = e.px / 2.24; // ship-local px per model unit
    const sliceH = e.px * (tex.frame.height / Math.max(1, tex.frame.width));
    const fullH = sliceH / span;
    w.root.position.set(hx, hy);
    w.root.scale.set(zs, zs * tiltY); // flat art lies on the (tilted) plane
    w.root.zIndex = depth;
    w.spin.rotation = heading + Math.PI / 2; // art is nose-up
    w.sprite.texture = tex;
    w.sprite.width = e.px;
    w.sprite.height = sliceH;
    // The slice's centre in ship-local px (nose up), then its separation.
    w.sprite.position.set((0.5 - e.anchor[0]) * e.px + p.ox * k,
      ((bounds[i] + bounds[i + 1]) / 2 - e.anchor[1]) * fullH + p.oz * k);
    w.sprite.rotation = p.yaw;
    const shade = Math.round(255 * (1 - 0.7 * p.char));
    w.sprite.tint = (shade << 16) | (shade << 8) | shade;
    w.sprite.alpha = p.alpha;
  });
  for (let i = wreckUsed; i < wreckPool.length; i++) wreckPool[i].root.visible = false;
}

/// A hull-local point (model units: x starboard, z aft) on screen, through the
/// wreck's pose and the plane's squash. Shared scratch result.
const hullPt = { x: 0, y: 0 };
function hullPoint(hx: number, hy: number, heading: number, k: number, lx: number, lz: number, zs: number): typeof hullPt {
  const r = heading + Math.PI / 2, c = Math.cos(r), sn = Math.sin(r);
  const px = lx * k, py = lz * k; // ship-local px, nose up
  hullPt.x = hx + (px * c - py * sn) * zs;
  hullPt.y = hy + (px * sn + py * c) * zs * tiltY;
  return hullPt;
}

/// Everything a death leaves on the battlefield, drawn in 2D: a capital's
/// burning hull and internal blasts before it breaks, the blast itself, hot
/// break edges and lingering fires on the sections, the debris (metal that
/// glints as it tumbles, embers that cool), dust and a smoke cloud — later
/// shock fronts push the debris and smoke. All functions of replay time.
function drawAfterlife(tex: FxTextures, zs: number): void {
  const light = fxLight!, smoke = fxSmoke!, s0 = st!;
  const metal = fxDebris ?? smoke; // debris passes beneath hulls and wrecks
  const now = s0.round + s0.frac;
  const T0 = now;
  const events = deathTimeline();
  const stride = perfTier >= 3 ? 4 : perfTier >= 2 ? 2 : 1;
  const out = { x: 0, y: 0 };
  const toPx = SCALE * camZoom;
  for (const e of events) {
    if (e.T > now) break;
    const s = now - e.T, D = e.breachT - e.T, b = now - e.breachT;
    const k = e.px / 2.24;
    const hull = wreckFrame(e, s);
    const hx = sx(hull.x), hy = sy(hull.y), heading = hull.heading;
    const near = hx > -300 && hy > -300 && hx < CANVAS_W + 300 && hy < CANVAS_H + 300;
    const width = Math.max(3, e.px * 0.16) * zs;

    // 1. Before the breach a capital burns and cooks off along its length.
    if (D > 0 && s < D && near) {
      const heat = s / D;
      for (let f = 0; f < 3 + e.cls; f++) {
        const lz = (jitter(e.seed ^ Math.imul(f + 1, 0x51ed)) - 0.5) * 1.5;
        const lx = (jitter(e.seed ^ Math.imul(f + 1, 0x7a3b)) - 0.5) * 0.25;
        if (jitter(e.seed ^ Math.imul(f + 1, 0x19)) > 0.35 + heat) continue; // fires spread as it burns
        const p = hullPoint(hx, hy, heading, k, lx, lz, zs);
        const flick = reducedMotion ? 1 : 0.7 + 0.6 * noise1(e.seed ^ (f + 31), T0 * 11);
        const size = width * (1.2 + 1.4 * heat) * flick;
        light.draw(tex.hot, p.x, p.y, size, size, 0, 0xff6a22, 0.6);
        light.draw(tex.soft, p.x, p.y, size * 0.55, size * 0.55, 0, 0xffe2a8, 0.85);
      }
      const nInt = e.cls === 3 ? 7 : e.kind === "dreadnought" || e.kind === "bulk_freighter" ? 4 : 3;
      for (let j = 0; j < nInt; j++) {
        const at = D * (0.08 + 0.82 * (j + jitter(e.seed ^ Math.imul(j + 1, 0x3c1))) / nInt);
        const age = s - at;
        if (age < 0 || age >= 0.25 || modelFx.has(`internal:${e.key}:${j}`)) continue;
        const u = age / 0.25, burst = 1 - Math.pow(1 - u, 3);
        const p = hullPoint(hx, hy, heading, k, (jitter(e.seed ^ (j * 977)) - 0.5) * 0.3,
          (jitter(e.seed ^ (j * 7919)) - 0.5) * 1.6, zs);
        const px = p.x, py = p.y, r = width * (1.6 + 2.6 * burst) * (e.cls === 3 ? 1.4 : 1);
        light.draw(tex.soft, px, py, r * 2.2, r * 2.2, 0, 0xff5a26, 0.5 * (1 - u));
        light.draw(tex.hot, px, py, r, r, 0, 0xffb347, 0.85 * (1 - u));
        light.draw(tex.soft, px, py, r * 0.45, r * 0.45, 0, 0xffffff, 0.95 * (1 - u));
        for (let q = 0; q < 4; q++) {
          planeSpark(tex, px, py, jitter(e.seed ^ (j * 131 + q * 17)) * Math.PI * 2, r * 0.3, r * (0.8 + burst), 1.2 * zs, 0xffd37a, 0.8 * (1 - u));
        }
      }
    }

    // 2. The blast: at the breach for a capital, at the blow for anything lighter.
    const life = BLAST_LIFE[e.cls] * (e.cls === 3 ? 1.6 : 1);
    if (b < 0) continue;
    const c = wreckFrame(e, D);
    const bx = c.x, by = c.y;
    const decay = Math.exp(-HULL_DRAG * D), vbx = e.vx * decay, vby = e.vy * decay;
    if (b < life) drawBlast(tex, sx(bx), sy(by), e.cls, b / life, `${s0.rec.id}:death:${e.key}`, zs, modelFx.has(`death:${e.key}`));

    // 3. Hot break edges cool; capital sections keep burning and venting.
    if (e.pieces >= 2 && near) {
      const ember = emberColor(b * 0.9);
      for (let cut = 0; cut < e.cuts.length && ember.glow > 0; cut++) {
        const a = pieceState(e, cut, s, b), z = pieceState(e, cut + 1, s, b);
        const lz = -1 + 2 * e.cuts[cut];
        const p = hullPoint(hx, hy, heading, k, (a.ox + z.ox) / 2, lz + (a.oz + z.oz) / 2, zs);
        const size = width * 2.2 * (0.6 + 0.4 * ember.glow);
        light.draw(tex.soft, p.x, p.y, size, size * 0.7, heading, ember.tint, 0.85 * ember.glow * Math.min(a.alpha, z.alpha));
      }
      if (e.cls >= 2 && b < 12) {
        const burn = 1 - b / 12;
        for (let i = 0; i < e.pieces; i++) {
          const pc = pieceState(e, i, s, b);
          if (pc.alpha <= 0.05) continue;
          const bounds = [0, ...e.cuts, 1];
          const lz = -1 + 2 * (bounds[i] + (bounds[i + 1] - bounds[i]) * jitter(e.seed ^ (i * 4099))) + pc.oz;
          const p = hullPoint(hx, hy, heading, k, pc.ox + (jitter(e.seed ^ (i * 271)) - 0.5) * 0.2, lz, zs);
          const fx0 = p.x, fy0 = p.y;
          const flick = reducedMotion ? 1 : 0.7 + 0.6 * noise1(e.seed ^ (i + 97), T0 * 9);
          const size = width * 1.4 * burn * flick;
          light.draw(tex.hot, fx0, fy0, size, size, 0, 0xff6a22, 0.55 * pc.alpha);
          light.draw(tex.soft, fx0, fy0, size * 0.5, size * 0.5, 0, 0xffe0a0, 0.8 * pc.alpha);
          // Venting smoke drifts off each burning section.
          const rate = 3, phase = jitter(e.seed ^ (i * 53));
          const last = Math.floor(b * rate - phase);
          for (let j = Math.max(0, Math.floor((b - 1.2) * rate - phase)); j <= last; j++) {
            const age = b - (j + phase) / rate;
            if (age < 0 || age > 1.2) continue;
            const ps = width * (1 + 3 * age) * burn;
            const dir = jitter(e.seed ^ Math.imul(j + 1, 0x2c9)) * Math.PI * 2;
            smoke.draw(tex.puff, fx0 + Math.cos(dir) * age * 10 * zs, fy0 + Math.sin(dir) * age * 10 * zs * tiltY - age * 6 * zs,
              ps, ps, dir, 0x6c6762, 0.32 * (1 - age / 1.2) * burn * pc.alpha);
          }
        }
      }
    }

    // 4. A lingering smoke cloud (lit from inside at first) and a wider dust haze.
    const base = [10, 16, 26, 40][e.cls] * zs;
    if (b < 2.6) {
      const n = 4 + 2 * e.cls;
      for (let j = 0; j < n; j += perfTier >= 2 ? 2 : 1) {
        const ang = jitter(e.seed ^ Math.imul(j + 1, 0x9e37)) * Math.PI * 2;
        const reach = 0.3 + jitter(e.seed ^ Math.imul(j + 1, 0x51)) * 0.9;
        const g = drift(b, DEBRIS_DRAG);
        pushed(bx + vbx * 0.5 * g + Math.cos(ang) * reach * base / toPx * (0.4 + Math.min(1, b)),
          by + vby * 0.5 * g + Math.sin(ang) * reach * base / toPx * (0.4 + Math.min(1, b)), e.breachT, now, events, out);
        const r = base * (0.45 + 0.8 * (1 - Math.exp(-b / 0.8)));
        const px = sx(out.x), py = sy(out.y);
        smoke.draw(tex.puff, px, py, r, r, ang + b * 0.2, j & 1 ? 0x34373c : 0x3f4247, 0.3 * Math.pow(1 - b / 2.6, 1.3));
        if (b < 0.5) light.draw(tex.soft, px, py, r * 0.9, r * 0.9, 0, 0xff7a30, 0.5 * (1 - b / 0.5));
      }
    }
    if (b < 5) {
      for (let j = 0; j < 2 + e.cls; j++) {
        const ang = jitter(e.seed ^ Math.imul(j + 7, 0x3f1)) * Math.PI * 2;
        pushed(bx + vbx * 0.4 * drift(b, DEBRIS_DRAG) + Math.cos(ang) * base / toPx, by + vby * 0.4 * drift(b, DEBRIS_DRAG) + Math.sin(ang) * base / toPx,
          e.breachT, now, events, out);
        const r = base * (0.8 + 1.8 * (1 - Math.exp(-b / 1.4)));
        smoke.draw(tex.puff, sx(out.x), sy(out.y), r, r * tiltY, ang, 0x6e6c6a, 0.08 * Math.exp(-b / 1.6));
      }
    }

    // 5. Debris cut from the hull's own art: plates and spars that tumble,
    // glow hot at first and catch the light; embers cool to dark.
    const g = drift(b, DEBRIS_DRAG);
    const art = debrisArt(e);
    const heat = clamp01(1 - b / 0.5);
    for (let j = 0; j < e.frags.length; j += stride) {
      const f = e.frags[j];
      pushed(bx + (vbx * 0.7 + Math.cos(f.angle) * f.speed) * g, by + (vby * 0.7 + Math.sin(f.angle) * f.speed) * g, e.breachT, now, events, out);
      const px = sx(out.x), py = sy(out.y);
      if (px < -60 || py < -60 || px > CANVAS_W + 60 || py > CANVAS_H + 60) continue;
      const size = f.size * Math.pow(zs, 0.9);
      const pick = (f.glint * 4099) | 0; // which cut this fragment is
      if (f.type === 2) {
        const ember = emberColor(b);
        if (ember.glow > 0.02) light.draw(tex.soft, px, py, size * (1.6 + 1.6 * ember.glow), size * (1.6 + 1.6 * ember.glow), 0, ember.tint, 0.9 * ember.glow);
        else if (art) metal.draw(art.plates[pick % art.plates.length], px, py, size, size, f.glint, 0x4a4542, 0.9);
        else metal.draw(tex.plate, px, py, size * 0.7, size * 0.7, f.glint, 0x2d2a28, 0.6);
        continue;
      }
      const rot = f.glint + f.spin * drift(b, 0.06);
      if (art) {
        // The piece turns over as it spins: it foreshortens and darkens
        // edge-on, and flashes as a face swings toward the light.
        const sliver = f.type === 1;
        const cut = sliver ? art.slivers[pick % art.slivers.length] : art.plates[pick % art.plates.length];
        const span = size * (sliver ? SLIVER_SPAN : PLATE_SPAN);
        const flip = Math.cos(f.glint * 3 + f.spin * 0.45 * drift(b, 0.06));
        const shade = Math.round(255 * (0.5 + 0.32 * Math.abs(flip)));
        metal.draw(cut, px, py, span, span * (0.3 + 0.7 * Math.abs(flip)), rot,
          mix(shade * 0x010101, 0xffb070, heat), 0.95);
        if (heat > 0) light.draw(tex.soft, px, py, span * 1.5, span * 1.5, 0, 0xff7a30, 0.45 * heat);
        const gl = Math.cos(rot * 2 - 0.9) * Math.abs(flip);
        if (gl > 0.93) light.draw(tex.flare, px, py, span * 1.7, span * 1.7, rot, 0xffffff, ((gl - 0.93) / 0.07) * 0.75);
        continue;
      }
      const aspect = 0.55 + 0.9 * ((f.glint * 7) % 1); // every plate tears differently
      metal.draw(f.type === 0 ? tex.plate : tex.strut, px, py, size * (f.type === 0 ? 1.1 * aspect : 1.8), size * (f.type === 0 ? 1.1 / aspect : 0.5),
        rot, b < 0.4 ? 0xffb066 : 0x59616b, 0.85);
      const gl = Math.cos(rot * 2 - 0.9);
      if (gl > 0.95) light.draw(tex.flare, px, py, size * 2.6, size * 2.6, rot, 0xffffff, ((gl - 0.95) / 0.05) * 0.75);
    }

    // 6. A titan's reactor lets go: the whole battlefield flashes.
    if (e.cls === 3 && b < 0.45) {
      const fl = Math.exp(-b / 0.1);
      const wash = Math.max(CANVAS_W, CANVAS_H) * 3;
      light.draw(tex.soft, sx(bx), sy(by), wash, wash, 0, 0xfff1d6, 0.6 * fl);
      const R = shockRadius(3, b) * toPx * 1.6;
      light.draw(tex.ring, sx(bx), sy(by), R * 2.3, R * 2.3 * tiltY, 0, 0xffe6b0, 0.85 * (1 - b / 0.45));
    }
  }
}

/// The staged detonation at a blast point: white ignition, boiling fire
/// lobes, an expanding shock ring, radial shards, then dark smoke. Size and
/// secondary-pop count follow the explosion class; `k` runs 0→1 over its
/// life; seeded geometry keeps it identical under scrubbing.
function drawBlast(tex: FxTextures, ex: number, ey: number, cls: 0 | 1 | 2 | 3, k: number, seedKey: string, zs: number,
  glowOnly = false): void {
  const light = fxLight!, smoke = fxSmoke!;
  const fade = 1 - k;
  const burst = 1 - Math.pow(1 - k, 3);
  const base = [12, 19, 30, 46][cls] * (cls === 3 ? 1.35 : 1);
  const rngD = mulberry32(hashId(seedKey));

  // Smoke blooms behind the fire after ignition, widening as the core collapses.
  // (When the 3D renderer shows the fireball, keep only the glow below.)
  const smokeT = clamp01((k - 0.14) / 0.86);
  const smokeN = glowOnly ? 0 : 4 + cls * 2;
  for (let si = 0; si < smokeN; si++) {
    const ang = rngD() * Math.PI * 2;
    const reach = 0.25 + rngD() * 0.65;
    const size = 0.28 + rngD() * 0.38;
    const r = base * reach * burst * zs;
    const sr = base * size * (0.5 + smokeT) * zs * 2.2;
    smoke.draw(tex.puff, ex + Math.cos(ang) * r, ey + Math.sin(ang) * r * tiltY, sr, sr, ang,
      si & 1 ? 0x2b2528 : 0x3b2b2a, 0.5 * smokeT * fade);
  }

  // Broad emissive bloom, then overlapping fire pockets igniting in turn.
  const bloom = base * (0.55 + 1.25 * burst) * zs * 2;
  light.draw(tex.soft, ex, ey, bloom, bloom, 0, 0xff4d24, 0.35 * fade);
  const fireN = glowOnly ? 0 : 6 + cls * 3;
  for (let li = 0; li < fireN; li++) {
    const ang = rngD() * Math.PI * 2;
    const phase = rngD() * (cls >= 2 ? 0.28 : 0.16);
    const reach = (0.12 + rngD() * 0.9) * burst;
    const size = 0.18 + rngD() * 0.34;
    const local = clamp01((k - phase) / Math.max(0.01, 1 - phase));
    if (local <= 0) continue;
    const lx = ex + Math.cos(ang) * base * reach * zs;
    const ly = ey + Math.sin(ang) * base * reach * zs * tiltY;
    const lr = base * size * (0.45 + 0.8 * local) * zs;
    const lf = (1 - local) * fade;
    light.draw(tex.soft, lx, ly, lr * 3.4, lr * 3.4, 0, 0xff5428, 0.4 * lf);
    light.draw(tex.hot, lx, ly, lr * 2, lr * 2, ang, 0xffa33d, 0.75 * lf);
    light.draw(tex.soft, lx, ly, lr * 1.1, lr * 1.1, 0, 0xfff0a6, 0.9 * lf);
  }

  // Capital hulls cook off in several offset secondary pops.
  const secondaryN = cls === 3 ? 6 : cls === 2 ? 3 : 0;
  for (let si = 0; si < secondaryN; si++) {
    const phase = 0.16 + rngD() * 0.34;
    const ang = rngD() * Math.PI * 2;
    const off = base * (0.25 + rngD() * 0.55) * zs;
    const local = clamp01((k - phase) / 0.24);
    if (local <= 0 || local >= 1) continue;
    const sf = Math.sin(local * Math.PI);
    const px = ex + Math.cos(ang) * off, py = ey + Math.sin(ang) * off * tiltY;
    const outer = base * (0.12 + 0.38 * local) * zs * 2.6, core = base * (0.06 + 0.16 * local) * zs * 2;
    light.draw(tex.soft, px, py, outer, outer, 0, 0xff6a2a, 0.6 * sf);
    light.draw(tex.soft, px, py, core, core, 0, 0xffffff, 0.9 * sf);
  }

  // The pressure front persists after the white core, so even light-hull
  // deaths read at overview zoom; it lies in the battlefield plane.
  const pr = base * (0.45 + 2.5 * burst) * zs;
  light.draw(tex.ring, ex, ey, pr * 2.3, pr * 2.3 * tiltY, 0, 0xffc56b, 0.7 * fade);

  // Long hot shards punch through the ring, every third ending in a glowing chunk.
  const sparkN = glowOnly ? 0 : 7 + cls * 4;
  for (let si = 0; si < sparkN; si++) {
    const ang = rngD() * Math.PI * 2;
    const reach = base * (0.75 + rngD() * 2.2) * burst * zs;
    const inner = reach * (0.34 + rngD() * 0.18);
    const hot = si % 3 === 0;
    planeSpark(tex, ex, ey, ang, inner, reach, (hot ? 2 : 1.4) * zs, hot ? 0xffffff : si & 1 ? 0xffd37a : 0xff6a32, 0.85 * fade);
    if (hot) {
      const chunk = 4 * zs * Math.max(0.5, fade);
      light.draw(tex.soft, ex + Math.cos(ang) * reach, ey + Math.sin(ang) * reach * tiltY, chunk, chunk, 0, 0xfff0b0, 0.85 * fade);
    }
  }

  // A very brief white-hot ignition sits on top of every other layer.
  const flash = clamp01(1 - k / 0.16);
  if (flash > 0) {
    const glow = base * (0.42 + 0.5 * burst) * zs * 2.4;
    const star = base * (1.05 + burst) * zs * 2;
    light.draw(tex.soft, ex, ey, glow, glow, 0, 0xffffff, 0.95 * flash);
    light.draw(tex.flare, ex, ey, star, star, 0, 0xfff3c4, 0.8 * flash);
    light.draw(tex.flare, ex, ey, star * 0.8, star * 0.8, Math.PI / 4, 0xfff3c4, 0.6 * flash);
  }
}

// --- FX rendering (pooled soft sprites; light is additive) ------------------------

/// Where a ship is along its recorded track at window fraction t (0..1): a
/// Catmull-Rom curve through the keyframes, exact at every round boundary,
/// carrying velocity across rounds (from the previous keyframe and, once its
/// light has arrived, the next), so fast hulls sweep and slow ones crawl
/// instead of every ship easing to a stop at each keyframe. Tangents are
/// capped by the neighbouring rounds' chords, so a hull that stops or starts
/// does so without overshooting. Also yields velocity and acceleration (arena
/// units per round, per round²). Returns a shared scratch object.
const motion = { x: 0, y: 0, vx: 0, vy: 0, ax: 0, ay: 0 };
function trackMotion(v: Pick<ShipVis, "x0" | "y0" | "x1" | "y1" | "vx0" | "vy0" | "vx2" | "vy2">, t: number): typeof motion {
  const dx = v.x1 - v.x0, dy = v.y1 - v.y0;
  const chord = Math.hypot(dx, dy);
  if (chord < 1e-6) {
    motion.x = v.x0; motion.y = v.y0;
    motion.vx = motion.vy = motion.ax = motion.ay = 0;
    return motion;
  }
  let m0x = (v.vx0 + dx) / 2, m0y = (v.vy0 + dy) / 2;
  let m1x = (dx + v.vx2) / 2, m1y = (dy + v.vy2) / 2;
  const cap0 = 1.5 * Math.min(chord, Math.hypot(v.vx0, v.vy0));
  const cap1 = 1.5 * Math.min(chord, Math.hypot(v.vx2, v.vy2));
  const l0 = Math.hypot(m0x, m0y), l1 = Math.hypot(m1x, m1y);
  if (l0 > cap0) { const k = l0 > 0 ? cap0 / l0 : 0; m0x *= k; m0y *= k; }
  if (l1 > cap1) { const k = l1 > 0 ? cap1 / l1 : 0; m1x *= k; m1y *= k; }
  const t2 = t * t, t3 = t2 * t;
  const h00 = 2 * t3 - 3 * t2 + 1, h10 = t3 - 2 * t2 + t, h01 = 1 - h00, h11 = t3 - t2;
  motion.x = h00 * v.x0 + h10 * m0x + h01 * v.x1 + h11 * m1x;
  motion.y = h00 * v.y0 + h10 * m0y + h01 * v.y1 + h11 * m1y;
  const d00 = 6 * t2 - 6 * t, d10 = 3 * t2 - 4 * t + 1, d11 = 3 * t2 - 2 * t;
  motion.vx = d00 * (v.x0 - v.x1) + d10 * m0x + d11 * m1x;
  motion.vy = d00 * (v.y0 - v.y1) + d10 * m0y + d11 * m1y;
  const a00 = 12 * t - 6, a10 = 6 * t - 4, a11 = 6 * t - 2;
  motion.ax = a00 * (v.x0 - v.x1) + a10 * m0x + a11 * m1x;
  motion.ay = a00 * (v.y0 - v.y1) + a10 * m0y + a11 * m1y;
  return motion;
}

const shipXY = (i: number, frac: number): [number, number] => {
  const v = st!.ships[i];
  if (!v) return [0, 0];
  const m = trackMotion(v, frac);
  return [m.x, m.y];
};

/** The only input to 3D: already-presented hulls and already-evidenced FX.
 * Explosion positions freeze at the impact instant, so fragments never follow
 * the surviving target. Nothing derives additional shots, losses or positions. */
function battleModelFrame(): BattleModelFrame {
  const bursts: BattleBurst[] = [], zs = Math.pow(camZoom, .85);
  const fx = st!.fx, f = st!.frac;
  /** x, y in canvas px. */
  const addAt = (id: string, type: BattleBurst["type"], x: number, y: number,
    time: number, life: number, radius: number, color: number, cls = 0) => {
    if (f < time || f >= time + life) return;
    bursts.push({ id, seed: hashId(`${st!.rec.id}:${st!.round}:${id}`), type,
      x, y, age: f - time, life, radius: radius * zs, color, cls });
  };
  /** x, y in arena units. */
  const add = (id: string, type: BattleBurst["type"], x: number, y: number,
    time: number, life: number, radius: number, color: number, cls = 0) =>
    addAt(id, type, sx(x), sy(y), time, life, radius, color, cls);
  if (fx) {
    fx.beams.forEach((b, i) => {
      const color = sideLaserColor(st!.ships[b.from]?.side ?? 0);
      if ((!b.miss || perfTier < 1) && f >= b.t && f < b.t + .045) {
        const [tx, ty] = shipXY(b.to, f);
        const m = muzzle(b.from, sx(tx), sy(ty), i, zs); // the barrel the 2D bolt leaves
        addAt(`muzzle:beam:${i}`, "muzzle", m.x, m.y, b.t, .045, b.heavy ? 8 : 3, color);
      }
      if (b.miss || b.glint) return; // no impact on a miss; keep reflective feedback
      const impact = b.t + (b.heavy ? 0 : .14 / 1.16);
      const [x, y] = shipXY(b.to, impact);
      add(`beam:${i}`, "impact", x, y, impact, .13, b.heavy ? 10 : 5, color);
    });
    fx.tracers.forEach((tr, i) => {
      if (tr.miss || tr.spall) return; // Whipple spall retains its distinct grammar
      const [x, y] = shipXY(tr.to, tr.t + .1);
      add(`driver:${i}`, "impact", x, y, tr.t + .1, .12, 4 + tr.strength, 0xffb95e);
    });
    fx.arcs.forEach((a, i) => {
      if (a.outcome !== "hit" && a.outcome !== "flak") return;
      const [x, y] = a.outcome === "flak" ? arcPoint(a, .72, a.tEnd) : shipXY(a.to, a.tEnd);
      add(`torpedo:${i}`, a.outcome === "hit" ? "impact" : "flak", x, y, a.tEnd,
        a.outcome === "hit" ? .2 : .12, a.outcome === "hit" ? 18 : 7, 0xffb363);
    });
    for (const t of fx.torps) {
      if (t.end !== "hit" && t.end !== "intercepted" && t.end !== "wreck") continue;
      const head = torpHead(t, t.tEnd);
      if (!head) continue;
      const warhead = t.end !== "intercepted"; // a hit, or a strike on wreckage
      add(`torp:${t.id}`, warhead ? "impact" : "flak", head[0], head[1], t.tEnd, warhead ? .2 : .12, warhead ? 18 : 7, 0xffb363);
    }
    fx.rams.forEach((r, i) => add(`ram:${i}`, "impact", r.x, r.y, r.t, .14, 5 + Math.min(10, Math.sqrt(r.damage) * .6), 0xffc070));
  }
  // Deaths: a capital's internal blasts, then the breach (or the blow itself).
  const now = st!.round + f;
  for (const e of deathTimeline()) {
    if (e.T > now) break;
    const D = e.breachT - e.T, life = BLAST_LIFE[e.cls] * (e.cls === 3 ? 1.6 : 1);
    if (now >= e.breachT && now < e.breachT + life) {
      const c = wreckFrame(e, D);
      addAt(`death:${e.key}`, "death", sx(c.x), sy(c.y), e.breachT - st!.round, life,
        BLAST_RADIUS[e.cls] * (e.cls === 3 ? 1.35 : 1), 0xff8136, e.cls);
    }
    if (D > 0 && now < e.breachT) {
      const s = now - e.T, hull = wreckFrame(e, s), hx = sx(hull.x), hy = sy(hull.y), heading = hull.heading;
      const nInt = e.cls === 3 ? 7 : e.kind === "dreadnought" || e.kind === "bulk_freighter" ? 4 : 3;
      for (let j = 0; j < nInt; j++) {
        const at = D * (0.08 + 0.82 * (j + jitter(e.seed ^ Math.imul(j + 1, 0x3c1))) / nInt);
        if (s < at || s >= at + 0.25) continue;
        const p = hullPoint(hx, hy, heading, e.px / 2.24, (jitter(e.seed ^ (j * 977)) - 0.5) * 0.3,
          (jitter(e.seed ^ (j * 7919)) - 0.5) * 1.6, zs);
        addAt(`internal:${e.key}:${j}`, "impact", p.x, p.y, e.T + at - st!.round, 0.25,
          BLAST_RADIUS[e.cls] * 0.45, 0xff9a3c);
      }
    }
  }
  return { width: CANVAS_W, height: CANVAS_H, tier: perfTier, reducedMotion, bursts, wrecks: wreckPieces,
    pitch: hullPitch(), ringTilt: ringTilt(), depthSlope: depthSlope(),
    ships: st!.ships.flatMap((v, id) => v.inUse ? [{ id, kind: v.kind, x: v.root.position.x,
      y: v.root.position.y, angle: screenHeading(v.spin.rotation - Math.PI / 2), canvasPx: v.modelPx * zs,
      alpha: v.root.alpha, eligible: v.modelEligible }] : []) };
}

/// Quadratic bezier point for a torpedo arc (curving pursuit).
function arcPoint(a: FxWindow["arcs"][number], t: number, frac: number): [number, number] {
  const [tx, ty] = shipXY(a.to, frac);
  const u = 1 - t;
  return [u * u * a.x0 + 2 * u * t * a.cx + t * t * tx, u * u * a.y0 + 2 * u * t * a.cy + t * t * ty];
}

/// An arc's current HEAD position at transport time `frac` (flak arcs die at
/// 72% of the path) — the one shared formula for drawing, PD fans, and hover.
function arcHead(a: FxWindow["arcs"][number], frac: number): [number, number] {
  const t = Math.min(frac / a.tEnd, 1) * (a.outcome === "flak" ? 0.72 : 1);
  return arcPoint(a, t, frac);
}

/// Where a recorded torpedo is at window time `f` (arena units), or null before
/// it appears and once it is gone. It flies straight at its real speed; a hit
/// strikes the hull where it is drawn; a miss coasts on past.
function torpHead(t: TorpTrack, f: number): [number, number] | null {
  if (f < t.t0) return null;
  if (t.end === "launch") return t.from !== null ? shipXY(t.from, f) : [t.x0, t.y0];
  if (t.end === "fly") return [t.x0 + (t.x1 - t.x0) * f, t.y0 + (t.y1 - t.y0) * f];
  if (t.end === "dry" || t.end === "fade") return f <= t.tEnd ? [t.x0, t.y0] : null;
  let ex = t.x1, ey = t.y1; // (a wreck strike flies to the recorded point on the wreck)
  if (t.end === "hit" && t.to !== null) {
    // Keep the recorded offset from the hull it strikes, wherever it is drawn.
    const [ax, ay] = shipXY(t.to, t.tEnd), [bx, by] = shipXY(t.to, 1);
    ex += ax - bx;
    ey += ay - by;
  }
  if (f <= t.tEnd) {
    const u = f / t.tEnd;
    return [t.x0 + (ex - t.x0) * u, t.y0 + (ey - t.y0) * u];
  }
  if (t.end === "miss" && f <= t.tEnd + MISS_COAST) {
    const dx = t.x1 - t.x0, dy = t.y1 - t.y0, d = Math.hypot(dx, dy) || 1;
    const k = (f - t.tEnd) * TORP_SPEED * 0.8;
    return [ex + (dx / d) * k, ey + (dy / d) * k];
  }
  return null;
}

function drawDebris(): void {
  if (!debrisG || !st) return;
  debrisG.clear();
  // The arena ring + withdraw edge — on the battlefield plane, so they track
  // the camera and foreshorten with the view angle.
  const ring = (r: number) => debrisG!.ellipse(sx(0), sy(0), r * SCALE * camZoom, r * SCALE * camZoom * tiltY);
  ring(ARENA_R).stroke({ color: 0x5a7ba6, alpha: 0.22, width: 1 });
  // The withdraw edge (the same at a world).
  ring(1400).stroke({ color: 0x5a7ba6, alpha: 0.1, width: 1 });
}

/// Ship-local pixels (nose up) → canvas, through the hull's current pose: the
/// same heading and plane squash the art and the 3D hull use. Returns a shared
/// scratch point (copy it before the next call — the no-allocation rule).
const fxPt = { x: 0, y: 0 };
function localToScreen(v: ShipVis, lx: number, ly: number, zs: number): { x: number; y: number } {
  const c = Math.cos(v.spin.rotation), s = Math.sin(v.spin.rotation);
  fxPt.x = v.root.position.x + (lx * c - ly * s) * zs;
  fxPt.y = v.root.position.y + (lx * s + ly * c) * zs * tiltY;
  return fxPt;
}

/// Where ship `i` fires from at a target on canvas (tx, ty): one of its three
/// guns nearest the target (the hull's real barrels), spread by shot index.
function muzzle(i: number, tx: number, ty: number, shot: number, zs: number): { x: number; y: number } {
  const v = st!.ships[i];
  if (!v || v.guns.length === 0) {
    fxPt.x = v?.root.position.x ?? tx;
    fxPt.y = v?.root.position.y ?? ty;
    return fxPt;
  }
  let g0 = 0, g1 = -1, g2 = -1, d0 = Infinity, d1 = Infinity, d2 = Infinity;
  for (let g = 0; g < v.guns.length; g++) {
    const p = localToScreen(v, v.guns[g][0], v.guns[g][1], zs);
    const d = (p.x - tx) * (p.x - tx) + (p.y - ty) * (p.y - ty);
    if (d < d0) { g2 = g1; d2 = d1; g1 = g0; d1 = d0; g0 = g; d0 = d; }
    else if (d < d1) { g2 = g1; d2 = d1; g1 = g; d1 = d; }
    else if (d < d2) { g2 = g; d2 = d; }
  }
  const pick = shot % (g2 >= 0 ? 3 : g1 >= 0 ? 2 : 1);
  const g = pick === 0 ? g0 : pick === 1 ? g1 : g2;
  return localToScreen(v, v.guns[g][0], v.guns[g][1], zs);
}

/// A ship's arena position at absolute replay time `t` (rounds): along this
/// window's curved track, or back along the previous window's for earlier times.
const arenaPt = { x: 0, y: 0 };
function shipArenaAt(v: ShipVis, t: number): { x: number; y: number } {
  const rel = t - st!.round;
  if (rel >= 0) {
    const m = trackMotion(v, Math.min(1, rel));
    arenaPt.x = m.x;
    arenaPt.y = m.y;
  } else {
    arenaPt.x = v.x0 + v.vx0 * rel;
    arenaPt.y = v.y0 + v.vy0 * rel;
  }
  return arenaPt;
}

const towardWhite = (color: number, t: number): number => {
  const r = (color >> 16) & 255, g = (color >> 8) & 255, b = color & 255;
  return (Math.round(r + (255 - r) * t) << 16) | (Math.round(g + (255 - g) * t) << 8) | Math.round(b + (255 - b) * t);
};

/// Two additive sprites per plume — a nozzle glow and its flame — live inside
/// the hull's own transform, under its art (and under the 3D hull), so the
/// hull hides each flame's root and the flame lies in the battlefield plane.
function ensureFlames(v: ShipVis): void {
  if (!fxTex) return;
  const need = v.plumes.length * 2;
  while (v.flames.length < need) {
    const nozzle = new Sprite(fxTex.soft);
    const flame = new Sprite(fxTex.plume);
    nozzle.anchor.set(0.5);
    flame.anchor.set(0.5, 0);
    nozzle.blendMode = flame.blendMode = "add";
    v.engines.addChild(flame, nozzle);
    v.flames.push(nozzle, flame);
  }
  for (let i = need; i < v.flames.length; i++) v.flames[i].visible = false;
}

/// Engine output follows the curved track: plume length and brightness rise
/// with speed and with acceleration, a withdrawing side burns flat out, and
/// the flicker is seeded noise over replay time.
function updateEngines(v: ShipVis): void {
  const s = st!;
  const n = v.plumes.length;
  if (!fxTex || v.plat || n === 0 || v.flames.length < 2 * n) {
    v.engines.visible = false;
    return;
  }
  v.engines.visible = true;
  const thrust = engineThrust(v.speed, v.burn, MAX_SPEED[v.kind] ?? 40, s.round >= s.withdrawFrom[v.side]);
  const tint = sideLaserColor(v.side);
  const hot = towardWhite(tint, 0.6);
  const T = s.round + s.frac;
  for (let i = 0; i < n; i++) {
    const p = v.plumes[i];
    const nozzle = v.flames[2 * i], flame = v.flames[2 * i + 1];
    const flick = reducedMotion ? 1 : 0.85 + 0.3 * noise1(v.fxSeed ^ Math.imul(i + 1, 0x27d4eb2f), T * 9);
    nozzle.visible = perfTier < 2; // ladder: flames only
    nozzle.position.set(p.x, p.y);
    nozzle.scale.set((p.r * (2.4 + 1.6 * thrust)) / nozzle.texture.width);
    nozzle.tint = hot;
    nozzle.alpha = 0.3 + 0.6 * thrust;
    flame.visible = true;
    flame.position.set(p.x, p.y - p.r * 0.3);
    flame.scale.set((p.r * 2.2 * (0.85 + 0.3 * thrust)) / flame.texture.width,
      (p.r * (2 + 12 * thrust) * flick) / flame.texture.height);
    flame.tint = tint;
    flame.alpha = (0.35 + 0.6 * thrust) * (0.9 + 0.1 * flick);
  }
}

// Light (beams, flames, sparks, flashes) is drawn as additive soft sprites and
// smoke as normal sprites beneath it. Every value is a function of (record,
// round, frac) and the camera — seeded jitter, never the wall clock — so
// pausing and scrubbing repeat the same frame.
function drawFx(dt: number): void {
  if (!st || !fxTex || !fxLight || !fxSmoke) return;
  void dt;
  fxLight.begin();
  fxSmoke.begin();
  fxDebris?.begin();
  const zs = Math.pow(camZoom, 0.85); // world-ish FX radii track the camera
  const wjit = hashId(`${st.rec.id}:${st.round}`) | 0;
  const fx = st.fx;
  if (fx) {
    drawBeams(fx, fxTex, zs, wjit);
    drawTracers(fx, fxTex, zs, wjit);
    drawTorpedoes(fx, fxTex, zs, wjit);
    drawTorpTracks(fx, fxTex, zs, wjit);
    drawPointDefense(fx, fxTex, zs, wjit);
  }
  drawAfterlife(fxTex, zs);
  if (fx) drawRams(fx, fxTex, zs, wjit);
  if (fx) drawLinks(fx, fxTex, zs, wjit);
  drawDamage(fxTex, zs);
  fxLight.end();
  fxSmoke.end();
  fxDebris?.end();
}

// TACKLE and WEBS (rules v11): an interceptor holding a hull that is trying
// to leave is a thin line in its side's colour with a bright pulse racing to
// the held hull; a stasis web is a fainter, wider line with slow beads.
function drawLinks(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const light = fxLight!, f = st!.frac;
  const line = (a: number, b: number, tint: number, alpha: number, width: number) => {
    const va = st!.ships[a], vb = st!.ships[b];
    if (!va?.inUse || !vb?.inUse || va.root.alpha < 0.2 || vb.root.alpha < 0.2) return null;
    const [x0, y0] = shipXY(a, f), [x1, y1] = shipXY(b, f); // arena units
    const ax = sx(x0), ay = sy(y0), bx = sx(x1), by = sy(y1);
    const dx = bx - ax, dy = by - ay, len = Math.hypot(dx, dy);
    if (len < 2) return null;
    const ang = Math.atan2(dy, dx);
    light.draw(tex.beam, ax, ay, len, width * zs, ang, tint, alpha, 0, 0.5);
    return { ax, ay, dx, dy, len, ang };
  };
  fx.webs.forEach((w, i) => {
    const seg = line(w.from, w.to, 0x9fd8ff, 0.14, 2.4);
    if (!seg) return;
    for (let k = 0; k < 3; k++) {
      const s = (f * 1.2 + k / 3 + jitter(wjit ^ Math.imul(i + 1, 977))) % 1;
      light.draw(tex.soft, seg.ax + seg.dx * s, seg.ay + seg.dy * s, 5 * zs, 5 * zs, 0, 0xc4ecff, 0.4);
    }
  });
  fx.tackles.forEach((t, i) => {
    const tint = sideLaserColor(st!.ships[t.from]?.side ?? 0);
    const seg = line(t.from, t.to, tint, 0.3, 1.2);
    if (!seg) return;
    const s = (f * 3 + jitter(wjit ^ Math.imul(i + 1, 7919))) % 1;
    light.draw(tex.spark, seg.ax + seg.dx * s, seg.ay + seg.dy * s, Math.min(seg.len * 0.3, 10 * zs), 1.4 * zs, seg.ang,
      towardWhite(tint, 0.5), 0.8, 1, 0.5);
    light.draw(tex.soft, seg.ax + seg.dx, seg.ay + seg.dy, 7 * zs, 7 * zs, 0, towardWhite(tint, 0.3), 0.35);
  });
}

// RAMS — a hull that ran into wreckage: a hot flash at the contact point and
// a spray of sparks, sized by the damage it took.
function drawRams(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const light = fxLight!, f = st!.frac;
  fx.rams.forEach((r, i) => {
    const since = f - r.t;
    if (since < 0 || since >= 0.16 || modelFx.has(`ram:${i}`)) return;
    const k = since / 0.16, size = (5 + Math.min(16, Math.sqrt(r.damage))) * zs;
    const px = sx(r.x), py = sy(r.y);
    light.draw(tex.soft, px, py, size * 3, size * 3, 0, 0xff8a3a, 0.5 * (1 - k));
    light.draw(tex.soft, px, py, size, size, 0, 0xffffff, 0.9 * (1 - k));
    for (let q = 0; q < 6; q++) {
      const seed = wjit ^ Math.imul(i + 1, 0x2d3f) ^ Math.imul(q + 1, 0x9e37);
      const reach = size * (1.2 + 2 * jitter(seed ^ 0x77)) * (0.4 + k);
      planeSpark(tex, px, py, jitter(seed) * Math.PI * 2, reach * 0.3, reach, 1.3 * zs, q & 1 ? 0xffc070 : 0xfff2cc, 0.85 * (1 - k));
    }
  });
}

/// A spark lying in the battlefield plane, radiating from (cx, cy) along the
/// plane angle `ang` between radii r0 and r1 (canvas px before the squash).
function planeSpark(tex: FxTextures, cx: number, cy: number, ang: number, r0: number, r1: number,
  width: number, tint: number, alpha: number): void {
  const c = Math.cos(ang), s = Math.sin(ang) * tiltY;
  fxLight!.draw(tex.spark, cx + c * r1, cy + s * r1, (r1 - r0) * Math.hypot(c, s), width, Math.atan2(s, c), tint, alpha, 1, 0.5);
}

// LASERS — travelling bolts of soft light (broad coloured haze, saturated
// body, white-hot core) leaving the shooter's own barrels. Capital lances draw
// light into the barrel, then bridge the full distance. A hit flares, throws
// sparks and leaves a brief glowing scorch; reflective plating glints instead.
function drawBeams(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const light = fxLight!, f = st!.frac;
  for (let bi = 0; bi < fx.beams.length; bi++) {
    const b = fx.beams[bi];
    if (b.miss && perfTier >= 1) continue;
    const dur = b.heavy ? 0.20 : 0.14;
    const dtb = f - b.t;
    if (dtb < (b.heavy ? -0.08 : 0) || dtb > dur + 0.34) continue;
    const color = sideLaserColor(st!.ships[b.from]?.side ?? 0);
    const [x1, y1] = shipXY(b.to, f);
    const tx0 = sx(x1), ty0 = sy(y1);
    const m = muzzle(b.from, tx0, ty0, bi, zs);
    const ax = m.x, ay = m.y;
    const seed = wjit ^ Math.imul(bi + 1, 0x45d9f3b);
    if (dtb < 0) {
      // Charging: a swelling glow, with motes of light drawn into the barrel.
      const charge = clamp01((dtb + 0.08) / 0.08);
      const g = (10 + 26 * charge) * zs, core = (3 + 8 * charge) * zs;
      light.draw(tex.soft, ax, ay, g, g, 0, color, 0.25 + 0.4 * charge);
      light.draw(tex.soft, ax, ay, core, core, 0, 0xffffff, 0.45 + 0.5 * charge);
      for (let k = 0; k < 4; k++) {
        const ang = jitter(seed ^ Math.imul(k + 1, 0x9e37)) * Math.PI * 2 + charge * 1.6;
        const rad = (16 - 13 * charge) * zs;
        light.draw(tex.soft, ax + Math.cos(ang) * rad, ay + Math.sin(ang) * rad * tiltY, 3.2 * zs, 3.2 * zs, 0, color, 0.75 * charge);
      }
      continue;
    }
    if (dtb <= dur) {
      const p = clamp01(dtb / dur);
      const fade = 1 - Math.max(0, (p - 0.78) / 0.22);
      const [tx, ty] = b.miss
        ? missedEndpoint(ax, ay, tx0, ty0, spritePx(st!.ships[b.to].kind) * zs * 0.8 + 12, b.missSign)
        : [tx0, ty0];
      const dx = tx - ax, dy = ty - ay;
      const dist = Math.max(1, Math.hypot(dx, dy));
      const ux = dx / dist, uy = dy / dist;
      const headP = b.heavy ? 1 : Math.min(b.miss ? 1.35 : 1, p * (b.miss ? 1.5 : 1.16));
      const hx = ax + dx * headP, hy = ay + dy * headP;
      const boltLen = b.heavy ? dist : Math.min(dist * headP, (24 + b.w * 12) * zs);
      const bx = hx - ux * boltLen, by = hy - uy * boltLen;
      // Overview widths as before; beams thicken gently once the camera zooms in.
      const beamW = ((b.heavy ? 2.0 : 1.15) * b.w + 0.55) * Math.max(1, Math.sqrt(zs / 1.5));
      const ang = Math.atan2(uy, ux);
      light.draw(tex.beam, bx, by, boltLen, beamW * 6.5, ang, color, 0.18 * fade, 0, 0.5);
      light.draw(tex.beam, bx, by, boltLen, beamW * 3.2, ang, color, 0.5 * fade, 0, 0.5);
      light.draw(tex.beam, bx, by, boltLen, Math.max(1.2, beamW * 1.2), ang, 0xffffff, 0.95 * fade, 0, 0.5);
      light.draw(tex.soft, hx, hy, beamW * 6, beamW * 6, 0, color, 0.45 * fade);
      if (p < 0.22) {
        const mk = 1 - p / 0.22;
        const mr = (2.5 + b.w * 2.2) * mk * zs;
        light.draw(tex.soft, ax, ay, mr * 4, mr * 4, 0, color, 0.4 * mk);
        light.draw(tex.flare, ax, ay, mr * 3.2, mr * 3.2, jitter(seed) * Math.PI, 0xffffff, 0.9 * mk);
      }
      const impactP = b.miss ? 0 : b.heavy ? 1 : clamp01((p - 1 / 1.16) / (1 - 1 / 1.16));
      if (impactP > 0 && b.glint) {
        // Reflective mitigation: a mirror-flash glint, not a bloom.
        const gr = (4 + 7 * impactP) * zs;
        light.draw(tex.flare, tx, ty, gr * 2.6, gr * 2.6, Math.PI / 4, 0xffffff, 0.95 * fade);
      } else if (impactP > 0 && !modelFx.has(`beam:${bi}`)) {
        const hitFade = fade * (1 - impactP * 0.35);
        const gs = (6 + 14 * impactP + b.w * 3) * zs, core = (3 + 6 * impactP) * zs;
        light.draw(tex.soft, tx, ty, gs, gs, 0, color, 0.4 * hitFade);
        light.draw(tex.soft, tx, ty, core, core, 0, 0xffffff, 0.9 * hitFade);
        for (let si = 0; si < 6; si++) {
          const sa = jitter(seed ^ Math.imul(si + 1, 0x9e37)) * Math.PI * 2;
          const sr = (4 + jitter(seed ^ Math.imul(si + 1, 0x7f4a)) * 10) * impactP * zs;
          planeSpark(tex, tx, ty, sa, sr * 0.28, sr, 1.6 * zs, si & 1 ? color : 0xffffff, 0.85 * hitFade);
        }
      }
    }
    if (!b.miss && !b.glint) {
      // The scorch: the struck hull glows briefly where the beam landed.
      const since = f - (b.t + (b.heavy ? 0 : 0.14 / 1.16));
      if (since >= 0 && since < 0.34) {
        const e = 1 - since / 0.34;
        const s = (4 + 2 * b.w) * zs;
        light.draw(tex.soft, tx0, ty0, s, s, 0, 0xff8a3a, 0.55 * e * e);
      }
    }
  }
}

// DRIVERS — a bright slug with a motion-blurred streak, fired with a muzzle
// blast from the shooter's barrel. A hit sprays sparks back along the line of
// fire (Whipple armor sheds grey spall instead); misses carry on past.
function drawTracers(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const light = fxLight!, smoke = fxSmoke!, f = st!.frac;
  const FLIGHT = 0.1;
  for (let ti = 0; ti < fx.tracers.length; ti++) {
    const tr = fx.tracers[ti];
    if (tr.miss && perfTier >= 1) continue;
    const dtt = f - tr.t;
    if (dtt < 0 || dtt > FLIGHT + 0.06) continue;
    const [x1raw, y1raw] = shipXY(tr.to, f);
    const txr = sx(x1raw), tyr = sy(y1raw);
    const m = muzzle(tr.from, txr, tyr, ti, zs);
    const ox = m.x, oy = m.y;
    const [x1, y1] = tr.miss
      ? missedEndpoint(ox, oy, txr, tyr, spritePx(st!.ships[tr.to].kind) * zs * 0.8 + 12, tr.missSign)
      : [txr, tyr];
    const ang = Math.atan2(y1 - oy, x1 - ox);
    const p = Math.min(1, dtt / FLIGHT);
    if (p < 1 || tr.miss) {
      const reach = tr.miss ? 1.35 : 1;
      const q = Math.max(0, p - (perfTier >= 2 ? 0.1 : 0.16)); // ladder: shorter streak
      const hx = ox + (x1 - ox) * p * reach, hy = oy + (y1 - oy) * p * reach;
      const len = Math.max(6 * zs, Math.hypot(x1 - ox, y1 - oy) * (p - q) * reach * 1.6);
      const out = dtt > FLIGHT ? 1 - (dtt - FLIGHT) / 0.06 : 1;
      const head = (3 + 2 * tr.strength) * zs;
      light.draw(tex.spark, hx, hy, len, (1.8 + 1.6 * tr.strength) * zs, ang, 0xffb24d, 0.9 * out, 1, 0.5);
      light.draw(tex.soft, hx, hy, head, head, 0, 0xfff0c8, 0.9 * out);
    }
    if (dtt < 0.04) {
      const mk = 1 - dtt / 0.04;
      const blast = (10 + 6 * tr.strength) * zs;
      light.draw(tex.soft, ox + Math.cos(ang) * blast * 0.3, oy + Math.sin(ang) * blast * 0.3, blast, blast * 0.45, ang, 0xffd98a, 0.75 * mk);
      light.draw(tex.flare, ox, oy, 7 * zs, 7 * zs, ang, 0xffffff, 0.8 * mk);
    }
    if (!tr.miss && p >= 1 && !modelFx.has(`driver:${ti}`)) {
      const k = clamp01((dtt - FLIGHT) / 0.06);
      if (tr.spall) {
        for (let s = 0; s < 4; s++) {
          const jx = (jitter(wjit ^ tr.from * 131 ^ tr.to * 61 ^ s * 977) - 0.5) * 12;
          const jy = (jitter(wjit ^ tr.from * 137 ^ tr.to * 67 ^ s * 983) - 0.5) * 12;
          const size = (3.2 + 2.5 * k) * zs;
          smoke.draw(tex.puff, x1 + jx * zs, y1 + jy * zs * tiltY, size, size, s * 1.7, 0x9aa8ba, 0.75 * (1 - 0.5 * k));
        }
      } else {
        const g = (6 + 6 * tr.strength) * zs;
        light.draw(tex.soft, x1, y1, g, g, 0, 0xffc66e, 0.75 * (1 - k));
        const back = Math.atan2(-(y1 - oy) / tiltY, -(x1 - ox)); // back toward the gun, in the plane
        for (let s = 0; s < 5; s++) {
          const sa = back + (jitter(wjit ^ Math.imul(ti + 1, 0x2c1b3c6d) ^ Math.imul(s + 1, 0x9e37)) - 0.5) * 2.2;
          const len = (5 + 9 * jitter(wjit ^ Math.imul(ti + 1, 7919) ^ Math.imul(s + 1, 104729))) * (0.6 + tr.strength) * zs;
          planeSpark(tex, x1, y1, sa, len * 0.2 * k, len * (0.4 + 0.6 * k), 1.4 * zs, s & 1 ? 0xffe0a8 : 0xffb95e, 0.85 * (1 - k));
        }
      }
    }
  }
}

// TORPEDOES — curving pursuit arcs: a motor glowing in its side's colour,
// a hot wake and a smoke trail thinning behind it. Point-defense kills pop
// short of the target; hits detonate largest.
function drawTorpedoes(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const light = fxLight!, smoke = fxSmoke!, f = st!.frac, T = st!.round + f;
  const trailN = perfTier >= 2 ? 5 : 9;
  for (let ai = 0; ai < fx.arcs.length; ai++) {
    const a = fx.arcs[ai];
    if (f > a.tEnd + 0.18 && a.outcome !== "fly") continue;
    const t = Math.min(f / a.tEnd, 1) * (a.outcome === "flak" ? 0.72 : 1);
    const [hx, hy] = arcPoint(a, t, f);
    const hsx = sx(hx), hsy = sy(hy);
    if (f <= a.tEnd) {
      for (let k = trailN; k >= 1; k--) {
        const [px, py] = arcPoint(a, Math.max(0, t - 0.026 * k), f);
        const size = (2.6 + 0.6 * k) * zs;
        smoke.draw(tex.puff, sx(px), sy(py), size, size, k * 1.3 + ai, 0x8d93a0, 0.22 * (1 - k / (trailN + 1)));
        if (k <= 3) {
          const wake = (4.2 - 0.8 * k) * zs;
          light.draw(tex.soft, sx(px), sy(py), wake, wake, 0, 0xff8a4a, 0.5 * (1 - k / 4));
        }
      }
      const flick = reducedMotion ? 1 : 0.8 + 0.4 * noise1(wjit ^ Math.imul(ai + 1, 7919), T * 14);
      light.draw(tex.soft, hsx, hsy, 8 * zs * flick, 8 * zs * flick, 0, sideLaserColor(a.side), 0.6);
      light.draw(tex.soft, hsx, hsy, 3 * zs, 3 * zs, 0, 0xffffff, 0.95);
    }
    const since = f - a.tEnd;
    if (since >= 0 && since < 0.12 && a.outcome === "flak" && !modelFx.has(`torpedo:${ai}`)) {
      const k = since / 0.12;
      const fr = (3 + 26 * since) * zs; // a flak shock front lies in the plane
      light.draw(tex.ring, hsx, hsy, fr * 2.3, fr * 2.3 * tiltY, 0, 0xffd98a, 0.85 * (1 - k));
      light.draw(tex.soft, hsx, hsy, 12 * zs, 12 * zs, 0, 0xffe6a8, 0.7 * (1 - k));
      for (let s = 0; s < 4; s++) {
        planeSpark(tex, hsx, hsy, jitter(wjit ^ Math.imul(ai + 1, 31) ^ Math.imul(s + 1, 977)) * Math.PI * 2,
          2 * zs, (4 + 10 * k) * zs, 1.2 * zs, 0xffe0a8, 0.8 * (1 - k));
      }
    }
    if (since >= 0 && since < 0.18 && a.outcome === "hit" && !modelFx.has(`torpedo:${ai}`)) {
      const [tx2, ty2] = shipXY(a.to, f);
      const k = since / 0.18;
      const burst = 1 - Math.pow(1 - k, 3);
      const ex = sx(tx2), ey = sy(ty2);
      const outer = (8 + 48 * burst) * zs, mid = (4 + 24 * burst) * zs, core = (2.4 + 10 * burst) * zs;
      light.draw(tex.soft, ex, ey, outer, outer, 0, 0xff6a32, 0.45 * (1 - k));
      light.draw(tex.hot, ex, ey, mid, mid, 0, 0xffc05c, 0.7 * (1 - k));
      light.draw(tex.soft, ex, ey, core, core, 0, 0xffffff, 0.95 * (1 - k));
      const hr = (5 + 36 * burst) * zs; // the shock front lies in the plane
      light.draw(tex.ring, ex, ey, hr * 2.3, hr * 2.3 * tiltY, 0, 0xffd98a, 0.75 * (1 - k));
      for (let si = 0; si < 7; si++) {
        const seed = wjit ^ Math.imul(a.to + 1, 0x1f123bb5) ^ Math.imul(si + 1, 0x9e37);
        const rr = (8 + jitter(seed ^ 0x55aa) * 20) * burst * zs;
        planeSpark(tex, ex, ey, jitter(seed) * Math.PI * 2, rr * 0.35, rr, 1.5 * zs, si & 1 ? 0xff9a45 : 0xfff2cc, 0.8 * (1 - k));
      }
    }
  }
}

// RECORDED TORPEDOES — each flies its real path at its real speed: a motor
// glowing in its side's colour, a hot wake and a smoke trail thinning behind
// it. A launch kindles on its launcher; point defense pops one short of its
// target; a hit detonates on the hull; a miss coasts on and gutters out; a
// torpedo whose target is gone runs dry and fades.
function drawTorpTracks(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const light = fxLight!, smoke = fxSmoke!, f = st!.frac, T = st!.round + f;
  const trailN = perfTier >= 2 ? 5 : 9;
  for (const t of fx.torps) {
    const key = t.id % 65521;
    const since = f - t.tEnd;
    const head = torpHead(t, f);
    if (head) {
      const [hx, hy] = head;
      const gutter = t.end === "dry" || t.end === "fade" ? 1 - f / t.tEnd
        : t.end === "miss" && since > 0 ? 1 - since / MISS_COAST : 1;
      const a = Math.max(0, gutter) * (t.end === "launch" ? clamp01((f - t.t0) / 0.12) : 1);
      // The trail runs back along its heading, as long as it has flown.
      const prev = torpHead(t, Math.max(t.t0, f - 0.05)) ?? head;
      let dx = hx - prev[0], dy = hy - prev[1];
      if (Math.hypot(dx, dy) < 1e-6) { dx = t.x1 - t.x0; dy = t.y1 - t.y0; }
      const dl = Math.hypot(dx, dy);
      const trail = t.end === "launch" ? 0 : Math.min(TORP_TRAIL, t.trail0 + Math.hypot(hx - t.x0, hy - t.y0));
      if (dl > 1e-6 && trail > 0) {
        const ux = dx / dl, uy = dy / dl;
        for (let k = trailN; k >= 1; k--) {
          const back = (trail * k) / trailN;
          const px = sx(hx - ux * back), py = sy(hy - uy * back);
          const size = (2.6 + 0.6 * k) * zs;
          smoke.draw(tex.puff, px, py, size, size, k * 1.3 + key, 0x8d93a0, 0.22 * (1 - k / (trailN + 1)) * a);
          if (k <= 3) {
            const wake = (4.2 - 0.8 * k) * zs;
            light.draw(tex.soft, px, py, wake, wake, 0, 0xff8a4a, 0.5 * (1 - k / 4) * a);
          }
        }
      }
      const hsx = sx(hx), hsy = sy(hy);
      const flick = reducedMotion ? 1 : 0.8 + 0.4 * noise1(wjit ^ Math.imul(key + 1, 7919), T * 14);
      light.draw(tex.soft, hsx, hsy, 8 * zs * flick, 8 * zs * flick, 0, sideLaserColor(t.side), 0.6 * a);
      light.draw(tex.soft, hsx, hsy, 3 * zs, 3 * zs, 0, 0xffffff, 0.95 * a);
      if (t.end === "launch" && f - t.t0 < 0.1) {
        const k = (f - t.t0) / 0.1; // the launch flash on the hull
        light.draw(tex.flare, hsx, hsy, 14 * zs, 14 * zs, 0, 0xffe2b0, 0.7 * (1 - k));
      }
    }
    if (since >= 0 && since < 0.12 && t.end === "intercepted" && !modelFx.has(`torp:${t.id}`)) {
      const at = torpHead(t, t.tEnd);
      if (at) {
        const hsx = sx(at[0]), hsy = sy(at[1]), k = since / 0.12;
        const fr = (3 + 26 * since) * zs; // a flak shock front lies in the plane
        light.draw(tex.ring, hsx, hsy, fr * 2.3, fr * 2.3 * tiltY, 0, 0xffd98a, 0.85 * (1 - k));
        light.draw(tex.soft, hsx, hsy, 12 * zs, 12 * zs, 0, 0xffe6a8, 0.7 * (1 - k));
        for (let q = 0; q < 4; q++) {
          planeSpark(tex, hsx, hsy, jitter(wjit ^ Math.imul(key + 1, 31) ^ Math.imul(q + 1, 977)) * Math.PI * 2,
            2 * zs, (4 + 10 * k) * zs, 1.2 * zs, 0xffe0a8, 0.8 * (1 - k));
        }
      }
    }
    if (since >= 0 && since < 0.18 && (t.end === "hit" || t.end === "wreck") && !modelFx.has(`torp:${t.id}`)) {
      const at = torpHead(t, t.tEnd);
      if (at) {
        // The blast rides the hull it struck (wreckage is where it struck).
        let [ex, ey] = at;
        if (t.end === "hit" && t.to !== null) {
          const [ax, ay] = shipXY(t.to, f), [bx, by] = shipXY(t.to, t.tEnd);
          ex += ax - bx;
          ey += ay - by;
        }
        const k = since / 0.18, burst = 1 - Math.pow(1 - k, 3);
        const px = sx(ex), py = sy(ey);
        const outer = (8 + 48 * burst) * zs, mid = (4 + 24 * burst) * zs, core = (2.4 + 10 * burst) * zs;
        light.draw(tex.soft, px, py, outer, outer, 0, 0xff6a32, 0.45 * (1 - k));
        light.draw(tex.hot, px, py, mid, mid, 0, 0xffc05c, 0.7 * (1 - k));
        light.draw(tex.soft, px, py, core, core, 0, 0xffffff, 0.95 * (1 - k));
        const hr = (5 + 36 * burst) * zs; // the shock front lies in the plane
        light.draw(tex.ring, px, py, hr * 2.3, hr * 2.3 * tiltY, 0, 0xffd98a, 0.75 * (1 - k));
        for (let q = 0; q < 7; q++) {
          const seed = wjit ^ Math.imul(key + 1, 0x1f123bb5) ^ Math.imul(q + 1, 0x9e37);
          const rr = (8 + jitter(seed ^ 0x55aa) * 20) * burst * zs;
          planeSpark(tex, px, py, jitter(seed) * Math.PI * 2, rr * 0.35, rr, 1.5 * zs, q & 1 ? 0xff9a45 : 0xfff2cc, 0.8 * (1 - k));
        }
      }
    }
  }
}

// POINT DEFENSE — screening ships hose tracer dashes from their nearest guns
// at torpedoes inside their bubble (a Dreadnought's fan is denser and
// longer-ranged), with a small flak pop at the end of each stream. Recorded
// torpedoes draw it from the record: every screen fires lightly on the enemy
// torpedoes inside its bubble (each rolls against it there every step), and
// the hull that actually shot one down hoses it just before it pops.
function drawPointDefense(fx: FxWindow, tex: FxTextures, zs: number, wjit: number): void {
  const f = st!.frac;
  const bucket = Math.floor(f * 24);
  for (const pi of fx.pdShips) {
    const v = st!.ships[pi];
    if (!v?.inUse) continue;
    const [px, py] = shipXY(pi, f);
    if (fx.torps.length) {
      const radius = v.plat || v.kind === "dreadnought" ? PD_RADIUS_HEAVY : PD_RADIUS;
      for (const t of fx.torps) {
        if (t.side === v.side || t.end === "launch" || t.end === "dry" || t.end === "fade") continue;
        if (t.end !== "fly" && f > t.tEnd) continue;
        const head = torpHead(t, f);
        if (!head || Math.hypot(head[0] - px, head[1] - py) > radius) continue;
        pdStream(tex, pi, head[0], head[1], t.id % 65521, 1, 0.6, zs, wjit, bucket, f);
      }
      continue;
    }
    const radius = v.kind === "dreadnought" ? 400 : 180;
    const dense = (v.kind === "dreadnought" ? 5 : 3) - (perfTier >= 2 ? 1 : 0);
    for (let ai = 0; ai < fx.arcs.length; ai++) {
      const a = fx.arcs[ai];
      if (a.side === v.side || f > a.tEnd) continue;
      const [hx, hy] = arcHead(a, f);
      if (Math.hypot(hx - px, hy - py) > radius) continue;
      pdStream(tex, pi, hx, hy, ai, dense, 1, zs, wjit, bucket, f);
    }
  }
  for (const t of fx.torps) {
    if (t.end !== "intercepted" || t.by === null || f > t.tEnd || f < t.tEnd - 0.3) continue;
    const head = torpHead(t, f);
    if (head) pdStream(tex, t.by, head[0], head[1], t.id % 65521, 3 - (perfTier >= 2 ? 1 : 0), 1, zs, wjit, bucket, f);
  }
}

/// `dense` tracer dashes from ship `pi`'s nearest gun at a torpedo at (hx, hy).
function pdStream(tex: FxTextures, pi: number, hx: number, hy: number, key: number, dense: number, alpha: number,
  zs: number, wjit: number, bucket: number, f: number): void {
  const light = fxLight!;
  const m = muzzle(pi, sx(hx), sy(hy), key, zs);
  const ox = m.x, oy = m.y;
  for (let k = 0; k < dense; k++) {
    const tx = sx(hx + (jitter(wjit ^ pi * 131 ^ bucket * 40503 ^ k * 9973 ^ key) - 0.5) * 30);
    const ty = sy(hy + (jitter(wjit ^ pi * 137 ^ bucket * 40503 ^ k * 9931 ^ key) - 0.5) * 30);
    const dx = tx - ox, dy = ty - oy;
    const len = Math.hypot(dx, dy);
    if (len < 1) continue;
    const ang = Math.atan2(dy, dx);
    const s = (f * 9 + jitter(wjit ^ pi * 977 ^ k * 7919)) % 1; // the dash races out
    light.draw(tex.beam, ox, oy, len, 1.6 * zs, ang, 0xbfe0ff, 0.1 * alpha, 0, 0.5);
    light.draw(tex.spark, ox + dx * s, oy + dy * s, Math.min(len * 0.5, 9 * zs), 1.3 * zs, ang, 0xd6ecff, 0.85 * alpha, 1, 0.5);
    if (k === 0) light.draw(tex.soft, tx, ty, 5 * zs, 5 * zs, 0, 0xffe7b0, 0.5 * alpha);
  }
}

// DAMAGE — a hull vents smoke as its recorded hull falls (one vent below 60%,
// two below 40%) and burns with sparks below 30%; every evidenced hit lights
// the whole hull for an instant. Puffs leave each vent on a fixed replay-time
// cadence from where the hull was then, so smoke trails behind a moving ship
// and scrubbing shows the same smoke.
function drawDamage(tex: FxTextures, zs: number): void {
  const light = fxLight!, smoke = fxSmoke!, s = st!, f = s.frac, T = s.round + f;
  const rate = perfTier >= 2 ? 4 : 7; // puffs per vent per round
  const LIFE = 0.9; // rounds
  const toPx = SCALE * camZoom;
  for (let i = 0; i < s.ships.length; i++) {
    const v = s.ships[i];
    if (!v.inUse || v.root.alpha < 0.05) continue;
    const cx = v.root.position.x, cy = v.root.position.y;
    if (cx < -120 || cy < -120 || cx > CANVAS_W + 120 || cy > CANVAS_H + 120) continue;
    const heading = v.spin.rotation - Math.PI / 2;
    let lastHit = -1;
    for (const h of v.hitTimes) if (h <= f && h > lastHit) lastHit = h;
    if (lastHit >= 0 && f - lastHit < 0.07) {
      const k = 1 - (f - lastHit) / 0.07;
      const c = Math.cos(heading), sn = Math.sin(heading) * tiltY;
      const along = 2 * v.halfL * zs * Math.hypot(c, sn);
      light.draw(tex.soft, cx, cy, along * 1.5, 2 * v.halfW * zs * 1.5, Math.atan2(sn, c), 0xffffff, 0.35 * k);
    }
    const vents = smokeVents(v.hp);
    if (vents === 0 || (perfTier >= 3 && !v.plat && (MASS[v.kind] ?? 0) < 2000)) continue;
    const [nowX, nowY] = shipXY(i, f);
    const puffBase = Math.max(3, (v.halfW + v.halfL) * 0.5) * zs;
    const burning = hullOnFire(v.hp);
    for (let e = 0; e < vents; e++) {
      const em = localToScreen(v, v.vents[e][0], v.vents[e][1], zs);
      const emx = em.x, emy = em.y;
      const phase = jitter(v.fxSeed ^ Math.imul(e + 1, 7919));
      const last = Math.floor(T * rate - phase);
      for (let j = Math.floor((T - LIFE) * rate - phase); j <= last; j++) {
        const born = (j + phase) / rate;
        const age = T - born;
        if (age < 0 || age > LIFE) continue;
        const u = age / LIFE;
        const then = shipArenaAt(v, born);
        const bx = emx - (nowX - then.x) * toPx, by = emy - (nowY - then.y) * toPx * tiltY;
        const seed = v.fxSeed ^ Math.imul(j + 1, 0x9e3779b1) ^ e;
        const dir = heading + Math.PI + (jitter(seed) - 0.5) * 1.3;
        const drift = (4 + 14 * u) * zs;
        const size = puffBase * (0.35 + 1.0 * u);
        smoke.draw(tex.puff, bx + Math.cos(dir) * drift, by + Math.sin(dir) * drift * tiltY, size, size,
          jitter(seed ^ 0x55aa) * Math.PI * 2 + u, j & 1 ? 0x7a756f : 0x5f5b57,
          0.3 * Math.min(1, age / 0.08) * Math.pow(1 - u, 1.3));
      }
      if (burning && e < 2) {
        const flick = reducedMotion ? 1 : 0.75 + 0.5 * noise1(v.fxSeed ^ (e + 11), T * 10);
        const fire = Math.max(3, v.halfW * 0.7) * zs * flick;
        light.draw(tex.hot, emx, emy, fire * 1.6, fire * 1.6, 0, 0xff6a22, 0.55);
        light.draw(tex.soft, emx, emy, fire * 0.8, fire * 0.8, 0, 0xffe0a0, 0.8);
        if (perfTier < 2) {
          // Three sparks a round leave each burning vent.
          const sj = Math.floor(T * 3 + phase);
          const sAge = T - (sj - phase) / 3;
          if (sAge >= 0 && sAge < 0.2) {
            const k = sAge / 0.2;
            planeSpark(tex, emx, emy, jitter(v.fxSeed ^ Math.imul(sj + 1, 31) ^ e) * Math.PI * 2,
              (2 + 6 * k) * zs, (4 + 14 * k) * zs, 1.2 * zs, 0xffd37a, 0.9 * (1 - k));
          }
        }
      }
    }
  }
}

/// Capital deaths shake the screen at their breach (a titan hardest), and a
/// titan's breach raises the flagship-name banner (mirrors the sim's news
/// headline). Pure functions of replay time — scrub-safe.
function applyShakeAndBanner(): void {
  if (!st || !app || !layers) return;
  const now = st.round + st.frac;
  let shake = 0;
  let titan: DeathEvent | null = null;
  for (const e of deathTimeline()) {
    if (e.T > now) break;
    const b = now - e.breachT;
    if (e.cls >= 2 && b >= 0 && b < 0.5) shake = Math.max(shake, (e.cls === 3 ? 6 : 2.4) * (1 - b / 0.5));
    if (e.cls === 3 && b >= 0 && b < 1.4) titan = e;
  }
  const sb = hashId(`${st.rec.id}:${st.round}`) ^ Math.floor(st.frac * 60);
  if (reducedMotion) shake = 0;
  app.stage.position.set(shake ? (jitter(sb) - 0.5) * 2 * shake : 0, shake ? (jitter(sb ^ 0x55aa) - 0.5) * 2 * shake : 0);
  if (banner) {
    banner.style.display = titan ? "block" : "none";
    if (titan) {
      if (bannerKey !== titan.key) {
        bannerKey = titan.key;
        const name = st.rec.sides[titan.side]?.flagship_name ?? "THE TITAN";
        banner.textContent = `☄ ${name.toUpperCase()} IS DOWN`;
      }
      banner.style.opacity = String(Math.max(0, 1 - Math.max(0, now - titan.breachT - 0.6) * 1.25));
    }
  }
}

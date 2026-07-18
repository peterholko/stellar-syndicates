// §theater — BATTLE THEATER: ship sprites & weapon FX over truth keyframes.
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
//     #battle-viewer overlay (the map's renderer is untouched), renders only
//     while the viewer is open, and pauses when the tab is hidden.
//
// Data reality (what the record actually carries, and what we derive):
//   * `KeyframeView.ships` have no id, velocity, or stack key → identity is
//     MATCHED between consecutive frames (nearest-neighbour within a
//     (side, kind, platform) group), headings come from position deltas, and
//     a representative's ×N badge is `side count of kind ÷ shown of kind`.
//   * `dealt` is a scalar per side → weapon-FAMILY volumes are derived from
//     the side's participant loadout stacks (each stack's offense family).
//   * torpedoes are ONE centroid salvo per side with a live count → the
//     theater expands them into individual cosmetic arcs; the round-to-round
//     count drop budgets how many arcs resolve (hit or intercepted).

import { Application, Assets, Container, Graphics, Sprite, Text, TextStyle, Texture } from "pixi.js";
import type { BattleRecordView, KeyframeView, ShipKind } from "./protocol";
import { hashId, mulberry32 } from "./prng";
import { starTypeFor, starConceptUrl } from "./stars";
import { STAR_TINT } from "./systemview";

// --- Geometry & scale ---------------------------------------------------------

/// Battle-local arena half-extent the theater frames (arena ring 1000 +
/// withdraw margin — mirrors the sim's WITHDRAW_EXIT_RADIUS 1400).
const ARENA_R = 1000;
const VIEW_R = 1500;
/// Canvas size — the viewer overlay is min(760px, 94vw) wide.
const CANVAS_W = 712;
const CANVAS_H = 448;
/// Arena → screen scale: fit the withdraw margin into the canvas height.
const SCALE = CANVAS_H / (2 * VIEW_R);

/// Client mirror of the sim's hull masses (also mirrored in main.ts — keep in
/// step with crates/sim/src/ship.rs::hull_mass).
const MASS: Record<ShipKind, number> = {
  convoy: 4500, raider: 200, corvette: 800, colony: 6000, scout: 80,
  destroyer: 2000, cruiser: 4000, battleship: 8000, dreadnought: 16000, titan: 32000,
};
const KIND_LABEL: Record<ShipKind, string> = {
  convoy: "Convoy", raider: "Raider", corvette: "Corvette", colony: "Colony Ship", scout: "Scout",
  destroyer: "Destroyer", cruiser: "Cruiser", battleship: "Battleship", dreadnought: "Dreadnought", titan: "Titan",
};

/// Sprite size ∝ hull_mass^0.4 (log-ish: a 40× Titan reads huge without
/// dwarfing the arena), clamped so a Corvette stays visibly a ship.
function spritePx(kind: ShipKind): number {
  return Math.max(10, Math.min(64, 7.0 * Math.pow((MASS[kind] ?? 800) / 100, 0.4)));
}

// Team identity — the map's exact conventions (render.ts COL_OWN / COL_OTHER;
// no invented palette). Applied as engine/rim glow + a light sprite tint.
const TINT_OWN = 0x4fc3ff;
const TINT_FOE = 0xff7a6b;

// --- Art: one resolver, art-or-fallback ---------------------------------------

/// Per-kind single-ship art (the map-layer fleet_* composites are NOT used —
/// the theater draws individuals). Every kind is mapped; a kind whose file is
/// missing (or mid-load) renders the procedural silhouette until the texture
/// resolves — dropping a future PNG at the mapped path lights it up with zero
/// code change.
const SHIP_ART: Record<ShipKind, string> = {
  convoy: "cargo_freighter.png",
  raider: "raider_attack_ship.png",
  corvette: "corvette_escort_ship.png",
  colony: "colony_ship.png",
  scout: "scout_utility_ship.png",
  destroyer: "destroyer_line_ship.png",
  cruiser: "cruiser_line_ship.png",
  battleship: "battleship_line_ship.png",
  dreadnought: "dreadnought_line_ship.png",
  titan: "titan_flagship.png",
};
const STATION_ART = "/art/celestial_sprites/mining_station.png";

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
      (t: Texture) => texCache.set(url, t),
      () => texCache.set(url, null), // missing art → permanent fallback
    );
  }
  return null;
}
const shipTexture = (kind: ShipKind): Texture | null => resolveTexture(`/art/ship_sprites/${SHIP_ART[kind]}`);

// --- Scene types ----------------------------------------------------------------

/// One tracked representative on screen (pooled — never allocated per frame).
interface ShipVis {
  root: Container;
  sprite: Sprite;
  body: Graphics; // procedural silhouette fallback
  glow: Graphics; // team engine glow
  badge: Text; // ×N representative count
  plate: Text; // Titan nameplate / platform label
  inUse: boolean;
  // matched track for the active window:
  x0: number; y0: number; x1: number; y1: number;
  hdg: number; // smoothed heading (rad)
  side: number;
  kind: ShipKind;
  plat: boolean;
  hp: number;
  reps: number; // hulls this sprite stands for
  seed: number; // per-ship cosmetic seed (idle drift phase)
  entered: boolean; // newly committed this window (wave arrival) → fade in
  exiting: boolean; // unmatched in the next frame (died/withdrew) → fade out
}

/// The FX SCHEDULE for one window — derived ONCE per (battleId, round) from
/// record data + the cosmetic PRNG, then rendered statelessly against the
/// transport clock. Scrubbing back re-derives the identical schedule.
interface FxWindow {
  /// Beam volleys: instantaneous flash-lines (they arrive with their own
  /// light — the instant line IS the fiction). `heavy` = capital emitter
  /// (charge-up + thicker, longer-held line; the Lance Array variant when
  /// that module lands). `glint` = target is Reflective-fitted (mirror-flash
  /// deflection instead of a full bloom).
  beams: { t: number; from: number; to: number; w: number; heavy: boolean; glint: boolean }[];
  /// Driver tracer streaks with short visible flight; `miss` tracers pass
  /// close and carry on (pure cosmetics — hit math already happened in the
  /// sim). `spall` = target is Whipple-fitted (shattered-armor debris puffs
  /// instead of clean sparks).
  tracers: { t: number; from: number; to: number; miss: boolean; spall: boolean }[];
  /// Torpedo arcs — the centerpiece motion. Expanded from the per-side salvo
  /// summary: each arc curves from the salvo origin toward its target across
  /// the window. outcome: 'fly' persists past the window; 'hit' detonates at
  /// tEnd; 'flak' dies to point-defense short of the target at tEnd.
  arcs: { side: number; x0: number; y0: number; cx: number; cy: number; to: number; tEnd: number; outcome: "fly" | "hit" | "flak" }[];
  /// PD tracer fans: which defending ships screen this window (indices).
  pdShips: number[];
  /// Exact deaths from the record, timed within the window by their step.
  deaths: { t: number; x: number; y: number; kind: ShipKind; side: number; cls: 0 | 1 | 2 | 3; shipIdx: number | null }[];
}

interface TheaterState {
  rec: BattleRecordView;
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
let layers: {
  backdrop: Container;
  debris: Container;
  ships: Container;
  fx: Container;
  ui: Container;
} | null = null;
let st: TheaterState | null = null;
let shipPool: ShipVis[] = [];
let backdropKey = ""; // rebuilt only when the record changes
let sceneClock = 0; // seconds since open — drives idle drift only (cosmetic)
// Persistent immediate-mode surfaces — cleared + redrawn each frame, never
// re-allocated (the budget law: pooled surfaces, no per-frame objects).
let fxG: Graphics | null = null;
let debrisG: Graphics | null = null;
let debrisKey = ""; // debris field rebuilt only when the round changes
let debrisField: { x: number; y: number; dx: number; dy: number; r: number; tint: number; bornRound: number }[] = [];
let banner: HTMLDivElement | null = null;
let bannerKey = "";
// Degradation ladder: rolling fps estimate → tier 0 full · 1 no miss-tracers
// · 2 thinned drivers · 3 reduced debris. Torpedo arcs, flak, deaths, and
// mitigation feedback are NEVER dropped (they carry information).
let fpsEma = 60;
let perfTier = 0;

const sx = (x: number) => CANVAS_W / 2 + x * SCALE;
const sy = (y: number) => CANVAS_H / 2 + y * SCALE;

/// Create the Pixi app once, lazily; reused across opens forever after.
async function ensureApp(): Promise<void> {
  if (app) return;
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
    resolution: window.devicePixelRatio || 1,
  });
  holder.appendChild(app.canvas);
  holder.appendChild(tooltip);
  layers = {
    backdrop: new Container(),
    debris: new Container(),
    ships: new Container(),
    fx: new Container(),
    ui: new Container(),
  };
  app.stage.addChild(layers.backdrop, layers.debris, layers.ships, layers.fx, layers.ui);
  debrisG = new Graphics();
  layers.debris.addChild(debrisG);
  fxG = new Graphics();
  layers.fx.addChild(fxG);
  banner = document.createElement("div");
  banner.className = "bv-theater-banner";
  banner.style.display = "none";
  holder.appendChild(banner);
  // Torpedo-arc hover: arcs are immediate-mode (no display objects), so the
  // canvas hit-tests the few live arc heads directly.
  app.canvas.addEventListener("pointermove", (ev: PointerEvent) => {
    if (!st?.fx || !tooltip || !app) return;
    const r = app.canvas.getBoundingClientRect();
    const mx = ((ev.clientX - r.left) / r.width) * CANVAS_W;
    const my = ((ev.clientY - r.top) / r.height) * CANVAS_H;
    for (const a of st.fx.arcs) {
      if (st.frac > a.tEnd) continue;
      const t = Math.min(st.frac / a.tEnd, 1) * (a.outcome === "flak" ? 0.72 : 1);
      const [hx, hy] = arcPoint(a, t, st.frac);
      if (Math.hypot(sx(hx) - mx, sy(hy) - my) < 10) {
        const target = st.ships[a.to];
        tooltip.textContent = `Torpedo salvo → ${target ? KIND_LABEL[target.kind] : "target"}`;
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
export function theaterAttach(mount: HTMLElement, rec: BattleRecordView): void {
  void ensureApp().then(() => {
    if (!holder || !app) return;
    if (holder.parentElement !== mount) mount.appendChild(holder);
    bindRecord(rec);
    app.ticker.start();
  });
}

/// The transport pushes time every animation frame (round + fractional
/// progress through it, whether playing, whether pinned LIGHT-LIVE).
export function theaterSetTime(round: number, frac: number, live: boolean): void {
  if (!st) return;
  st.round = round;
  st.frac = Math.max(0, Math.min(1, frac));
  st.live = live;
}

/// New light arrived for the open record (rounds grew) — rebind in place.
export function theaterRefresh(rec: BattleRecordView): void {
  if (st && st.rec.id === rec.id) {
    if (st.rec !== rec) st.windowKey = "";
    st.rec = rec;
  }
}

/// The viewer closed — stop rendering entirely (the map is never affected).
export function theaterClose(): void {
  st = null;
  if (app) app.ticker.stop();
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
    ticker: app.ticker.started,
    round: st.round,
    frac: st.frac,
    beams: st.fx?.beams.length ?? -1,
    tracers: st.fx?.tracers.length ?? -1,
    arcs: st.fx?.arcs.length ?? -1,
    arcFlak: st.fx?.arcs.filter((a) => a.outcome === "flak").length ?? -1,
    deaths: st.fx?.deaths.map((d) => ({ t: d.t, cls: d.cls, ship: d.shipIdx })) ?? null,
    debris: debrisField.length,
    tier: perfTier,
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
    h = (Math.imul(h, 31) + (v.side << 1) + (v.plat ? 1 : 0)) >>> 0;
    h = (Math.imul(h, 31) + Math.round(v.x0 * 10) + Math.round(v.y0 * 10)) >>> 0;
    h = (Math.imul(h, 31) + Math.round(v.x1 * 10) + Math.round(v.y1 * 10)) >>> 0;
    h = (Math.imul(h, 31) + v.reps) >>> 0;
  }
  return h >>> 0;
}

// --- Record binding & backdrop ---------------------------------------------------

function bindRecord(rec: BattleRecordView): void {
  if (st && st.rec.id === rec.id) {
    // Same battle, possibly NEW data (fresh light extended the rounds, or the
    // record object was replaced) — invalidate the active window so tracks
    // and the FX schedule rebuild against the new truth.
    if (st.rec !== rec) st.windowKey = "";
    st.rec = rec;
    return;
  }
  st = {
    rec,
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
  rec.rounds.forEach((r, i) => {
    for (const n of r.notes) {
      if ((n.kind === "retreat_tripped" || n.kind === "withdraw_ordered") && n.side !== null) {
        st!.withdrawFrom[n.side] = Math.min(st!.withdrawFrom[n.side], i);
      }
    }
  });
  sceneClock = 0;
  debrisKey = "";
  debrisField = [];
  bannerKey = "";
  buildBackdrop(rec);
}

/// System battles get the system's own visual identity — its star, faint and
/// parallax-far behind the arena; deep-space battles get a seeded starfield.
function buildBackdrop(rec: BattleRecordView): void {
  if (!layers) return;
  const key = `${rec.id}`;
  if (backdropKey === key) return;
  backdropKey = key;
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
  // The arena ring + withdraw edge, dashed-faint (the stage itself).
  const ring = new Graphics();
  ring.circle(CANVAS_W / 2, CANVAS_H / 2, ARENA_R * SCALE).stroke({ color: 0x5a7ba6, alpha: 0.22, width: 1 });
  ring.circle(CANVAS_W / 2, CANVAS_H / 2, 1400 * SCALE).stroke({ color: 0x5a7ba6, alpha: 0.1, width: 1 });
  layers.backdrop.addChild(ring);
  // The host system's star, if the battle stood at one.
  if (rec.system) {
    const t = starTypeFor(rec.system);
    const tint = STAR_TINT[t.slug] ?? 0xffe08a;
    const glow = new Graphics();
    glow.circle(CANVAS_W * 0.82, CANVAS_H * 0.2, 46).fill({ color: tint, alpha: 0.1 });
    glow.circle(CANVAS_W * 0.82, CANVAS_H * 0.2, 22).fill({ color: tint, alpha: 0.16 });
    layers.backdrop.addChild(glow);
    const url = starConceptUrl(t.slug);
    const tex = resolveTexture(url);
    const spr = new Sprite(tex ?? Texture.EMPTY);
    spr.anchor.set(0.5);
    spr.position.set(CANVAS_W * 0.82, CANVAS_H * 0.2);
    spr.alpha = 0.14; // parallax-faint — identity, not competition
    spr.width = 120;
    spr.height = 120;
    layers.backdrop.addChild(spr);
    if (!tex) {
      const swap = () => {
        const got = texCache.get(url);
        if (got) { spr.texture = got; spr.width = 120; spr.height = 120; }
        else if (got === undefined) setTimeout(swap, 300);
      };
      setTimeout(swap, 300);
    }
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
  root.addChild(glow, body, sprite, badge, plate);
  root.eventMode = "static";
  root.cursor = "default";
  const v: ShipVis = {
    root, sprite, body, glow, badge, plate,
    inUse: false, x0: 0, y0: 0, x1: 0, y1: 0, hdg: 0,
    side: 0, kind: "raider", plat: false, hp: 1, reps: 1, seed: 0,
    entered: false, exiting: false,
  };
  root.on("pointerover", () => showTip(v));
  root.on("pointermove", () => showTip(v));
  root.on("pointerout", hideTip);
  return v;
}

function acquireShip(): ShipVis {
  let v = shipPool.find((s) => !s.inUse);
  if (!v) {
    v = makeShipVis();
    shipPool.push(v);
    layers!.ships.addChild(v.root);
  }
  v.inUse = true;
  v.root.visible = true;
  return v;
}

function releaseAllShips(): void {
  for (const v of shipPool) {
    v.inUse = false;
    v.root.visible = false;
  }
}

function showTip(v: ShipVis): void {
  if (!tooltip || !st) return;
  const side = st.rec.sides[v.side];
  const fits = (side?.loadouts ?? []).filter((l) => l.kind === v.kind && l.modules.length > 0);
  const fitLine = fits.length ? fits.map((f) => f.modules.join("+")).join(" · ") : "unfitted";
  const label = v.plat ? "Defense Platform tier" : `${KIND_LABEL[v.kind]} ×${v.reps}`;
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

/// Match ships of frame A to frame B within (side, kind, plat) groups by
/// greedy nearest-neighbour — keyframes carry no ids, so identity is a
/// cosmetic reconstruction (good tracks for interpolation, not gameplay).
function matchFrames(a: KfShipView[], b: KfShipView[]): Array<{ from: KfShipView | null; to: KfShipView | null }> {
  const keyOf = (s: KfShipView) => `${s.side}:${s.kind}:${s.plat ? 1 : 0}`;
  const groups = new Map<string, { a: KfShipView[]; b: KfShipView[] }>();
  for (const s of a) {
    const g = groups.get(keyOf(s)) ?? { a: [], b: [] };
    g.a.push(s);
    groups.set(keyOf(s), g);
  }
  for (const s of b) {
    const g = groups.get(keyOf(s)) ?? { a: [], b: [] };
    g.b.push(s);
    groups.set(keyOf(s), g);
  }
  const out: Array<{ from: KfShipView | null; to: KfShipView | null }> = [];
  for (const g of groups.values()) {
    const unmatchedB = new Set(g.b);
    for (const s of g.a) {
      let best: KfShipView | null = null;
      let bestD = Infinity;
      for (const t of unmatchedB) {
        const d = (s.x - t.x) * (s.x - t.x) + (s.y - t.y) * (s.y - t.y);
        if (d < bestD) { bestD = d; best = t; }
      }
      if (best) {
        unmatchedB.delete(best);
        out.push({ from: s, to: best });
      } else {
        out.push({ from: s, to: null }); // died / withdrew / sampled out
      }
    }
    for (const t of unmatchedB) out.push({ from: null, to: t }); // wave arrival
  }
  return out;
}

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

// --- Window (re)build ------------------------------------------------------------------

function rebuildWindow(): void {
  if (!st || !layers) return;
  const { rec, round } = st;
  const key = `${rec.id}:${round}`;
  if (st.windowKey === key) return;
  st.windowKey = key;
  releaseAllShips();
  st.ships = [];
  const f0 = rec.rounds[round]?.frame;
  if (!f0) return;
  const f1 = rec.rounds[round + 1]?.frame ?? f0;
  const tracks = matchFrames(f0.ships, f1.ships);
  const reps = repsFor(rec, round, f0);
  const wseed = hashId(`${rec.id}:${round}`);
  let i = 0;
  for (const tr of tracks) {
    const src = tr.from ?? tr.to!;
    const v = acquireShip();
    v.side = src.side;
    v.kind = src.kind;
    v.plat = src.plat ?? false;
    v.hp = Math.max(0, Math.min(1, src.hp));
    v.reps = v.plat ? 1 : (reps.get(`${src.side}:${src.kind}`) ?? 1);
    v.x0 = (tr.from ?? tr.to!).x;
    v.y0 = (tr.from ?? tr.to!).y;
    v.x1 = (tr.to ?? tr.from!).x;
    v.y1 = (tr.to ?? tr.from!).y;
    v.entered = tr.from === null;
    v.exiting = tr.to === null;
    v.seed = (wseed ^ Math.imul(i + 1, 0x9e3779b9)) >>> 0;
    // Initial heading: along the track, else face the arena centre-line.
    const dx = v.x1 - v.x0, dy = v.y1 - v.y0;
    v.hdg = Math.abs(dx) + Math.abs(dy) > 1 ? Math.atan2(dy, dx) : (v.side === 0 ? 0 : Math.PI);
    dressShip(v);
    st.ships.push(v);
    i++;
  }
  st.fx = buildFxWindow(rec, round, st.ships, wseed);
  rebuildDebris();
}

// --- The FX schedule (all volume derives from the record) --------------------------

/// Weapon-family weights for a side, from its participant loadout stacks —
/// the wire has no dealt-by-family, so the fits ARE the family signal.
function familyWeights(rec: BattleRecordView, side: number): { beam: number; driver: number; torp: number; pdN: number; reflN: Map<ShipKind, number>; whipN: Map<ShipKind, number> } {
  let beam = 0.0001, driver = 0, torp = 0, pdN = 0;
  const reflN = new Map<ShipKind, number>();
  const whipN = new Map<ShipKind, number>();
  for (const stx of rec.sides[side]?.loadouts ?? []) {
    if (stx.modules.includes("torpedo_rack")) torp += stx.n;
    else if (stx.modules.includes("mass_driver")) driver += stx.n * 1.3;
    else beam += stx.n * (stx.modules.includes("point_defense_screen") ? 0.5 : 1);
    if (stx.modules.includes("point_defense_screen")) pdN += stx.n;
    if (stx.modules.includes("reflective_plating")) reflN.set(stx.kind, (reflN.get(stx.kind) ?? 0) + stx.n);
    if (stx.modules.includes("whipple_armor")) whipN.set(stx.kind, (whipN.get(stx.kind) ?? 0) + stx.n);
  }
  // Unfitted remainder fires stock beam: side counts minus fitted stacks.
  const rd = rec.rounds[0];
  const fitted = (rec.sides[side]?.loadouts ?? []).reduce((a, l) => a + l.n, 0);
  const total = (rd?.counts[side] ?? []).reduce((a, c) => a + (c.exact ?? 0), 0);
  beam += Math.max(0, total - fitted);
  return { beam, driver, torp, pdN, reflN, whipN };
}

/// Is the `idx`-th shown representative of its (side,kind) group treated as
/// carrying `n` fitted hulls? Deterministic: the FIRST ⌈share⌉ sprites of the
/// group wear the fit (stable order — every viewer sees the same ships glint).
function fittedFlag(ships: ShipVis[], v: ShipVis, fittedCount: number): boolean {
  if (fittedCount <= 0) return false;
  const group = ships.filter((s) => s.inUse && s.side === v.side && s.kind === v.kind && !s.plat);
  const pos = group.indexOf(v);
  const share = Math.ceil((fittedCount / Math.max(1, group.length * Math.max(1, v.reps))) * group.length);
  return pos >= 0 && pos < Math.max(1, Math.min(group.length, share));
}

function buildFxWindow(rec: BattleRecordView, round: number, ships: ShipVis[], wseed: number): FxWindow {
  const fx: FxWindow = { beams: [], tracers: [], arcs: [], pdShips: [], deaths: [] };
  const rng = mulberry32(wseed ^ 0x00f0f0);
  const rd = rec.rounds[round];
  const next = rec.rounds[round + 1];
  const alive = (side: number) => ships.map((v, i) => ({ v, i })).filter((e) => e.v.inUse && e.v.side === side);
  const wA = familyWeights(rec, 0);
  const wD = familyWeights(rec, 1);
  // Normalized intensity per side: this round's dealt vs the battle's peak —
  // fx volume visibly tracks the record's damage output.
  const dmax = Math.max(1e-6, ...rec.rounds.flatMap((r) => (r.dealt ? [r.dealt[0], r.dealt[1]] : [0])));
  for (let side = 0 as 0 | 1; side < 2; side = (side + 1) as 0 | 1) {
    const w = side === 0 ? wA : wD;
    const foes = alive(1 - side);
    const own = alive(side);
    if (!foes.length || !own.length) continue;
    const intensity = Math.min(1, (rd?.dealt?.[side] ?? 0) / dmax);
    const famTotal = w.beam + w.driver + w.torp;
    // Targets weighted by threat MASS (the sim's targeting spirit) — heavies
    // draw fire; the seeded roll keeps every viewer's scene identical.
    const pickTarget = () => {
      const tw = foes.map((f) => Math.pow(MASS[f.v.kind] ?? 800, 0.5));
      let r = rng() * tw.reduce((a, b) => a + b, 0);
      for (let k = 0; k < foes.length; k++) { r -= tw[k]; if (r <= 0) return foes[k].i; }
      return foes[foes.length - 1].i;
    };
    const reflFoe = side === 0 ? wD.reflN : wA.reflN;
    const whipFoe = side === 0 ? wD.whipN : wA.whipN;
    // BEAMS — count scaled by the beam share of this side's output.
    const nBeams = Math.round((1 + 7 * intensity) * (w.beam / famTotal));
    for (let k = 0; k < nBeams; k++) {
      const from = own[Math.floor(rng() * own.length)];
      const to = pickTarget();
      const tv = ships[to];
      fx.beams.push({
        t: 0.08 + rng() * 0.84,
        from: from.i,
        to,
        w: 0.8 + 2.6 * intensity * (w.beam / famTotal),
        heavy: (MASS[from.v.kind] ?? 0) >= 8000, // capitals fire the held lance-grade line
        glint: fittedFlag(ships, tv, reflFoe.get(tv.kind) ?? 0),
      });
    }
    // DRIVER tracers — bursty, short flight, seeded misses.
    const nTracers = Math.round((2 + 14 * intensity) * (w.driver / famTotal));
    for (let k = 0; k < nTracers; k++) {
      const from = own[Math.floor(rng() * own.length)];
      const to = pickTarget();
      const tv = ships[to];
      fx.tracers.push({
        t: 0.05 + rng() * 0.85,
        from: from.i,
        to,
        miss: rng() < 0.28,
        spall: fittedFlag(ships, tv, whipFoe.get(tv.kind) ?? 0),
      });
    }
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
        let dPd = Infinity;
        for (const pi of fx.pdShips) {
          const p = ships[pi];
          dPd = Math.min(dPd, Math.hypot(mx - p.x1, my - p.y1));
        }
        arcs.push({ d: dPd, a: { side, x0: s0.x + ox, y0: s0.y + oy, cx: mx, cy: my, to, tEnd, outcome: isResolved ? "hit" : "fly" } });
      }
      arcs.sort((p, q) => p.d - q.d);
      let flakLeft = nFlak;
      for (const e of arcs) {
        if (flakLeft > 0 && e.a.outcome === "hit") { e.a.outcome = "flak"; flakLeft--; }
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
        if (v.inUse && v.exiting && v.side === d.side && v.kind === d.kind) {
          const dist = Math.hypot(v.x0 - d.x, v.y0 - d.y);
          if (dist < best) { best = dist; shipIdx = i; }
        }
      });
      fx.deaths.push({ t: hi > lo ? 0.15 + 0.7 * ((d.step - lo) / (hi - lo)) : 0.5, x: d.x, y: d.y, kind: d.kind, side: d.side, cls, shipIdx });
    }
  }
  return fx;
}

/// The persistent battlefield: every death ≤ the current round leaves
/// drifting debris for the rest of the scene — the aftermath tableau is a
/// true battlefield. Deterministic: rebuilt from the record on round change;
/// drift is a pure function of (round + frac − bornRound).
function rebuildDebris(): void {
  if (!st) return;
  const key = `${st.rec.id}:${st.round}`;
  if (debrisKey === key) return;
  debrisKey = key;
  debrisField = [];
  const cap = perfTier >= 3 ? 4 : 9; // ladder tier 3: reduce debris counts
  for (let r = 0; r <= st.round && r < st.rec.rounds.length; r++) {
    for (const d of st.rec.rounds[r].frame?.deaths ?? []) {
      const rng = mulberry32(hashId(`${st.rec.id}:debris:${r}:${Math.round(d.x)}:${Math.round(d.y)}`));
      const mass = MASS[d.kind] ?? 800;
      const n = Math.min(cap, Math.max(3, Math.round(Math.pow(mass / 300, 0.45))));
      for (let k = 0; k < n; k++) {
        const ang = rng() * Math.PI * 2;
        const sp = 6 + rng() * 22;
        debrisField.push({
          x: d.x, y: d.y,
          dx: Math.cos(ang) * sp, dy: Math.sin(ang) * sp,
          r: 0.7 + rng() * (mass >= 8000 ? 2.6 : 1.4),
          tint: rng() < 0.35 ? 0xff9d5c : 0x8fa4bd, // some sections still burn
          bornRound: r,
        });
      }
    }
  }
}

/// Apply art/fallback, scale, team glow, badge, nameplate to a pooled sprite.
function dressShip(v: ShipVis): void {
  if (!st) return;
  const own = st.rec.own_side;
  const mine = own !== null && v.side === own;
  const tint = mine ? TINT_OWN : TINT_FOE;
  const px = v.plat ? 26 : spritePx(v.kind);
  const tex = v.plat ? resolveTexture(STATION_ART) : shipTexture(v.kind);
  if (tex) {
    v.sprite.texture = tex;
    v.sprite.visible = true;
    const ratio = tex.height > 0 ? tex.height / tex.width : 1;
    v.sprite.width = px;
    v.sprite.height = px * ratio;
    v.sprite.tint = 0xffffff;
    v.body.clear();
  } else {
    // Procedural silhouette, mass-scaled — the fallback idiom.
    v.sprite.visible = false;
    v.body.clear();
    v.body.poly([px * 0.55, 0, -px * 0.45, px * 0.34, -px * 0.28, 0, -px * 0.45, -px * 0.34])
      .fill({ color: 0xc9d6e8, alpha: 0.92 })
      .stroke({ color: tint, width: 1, alpha: 0.9 });
  }
  // Team identity: engine/rim glow in the side color (art stays natural).
  v.glow.clear();
  v.glow.circle(0, 0, px * 0.5).fill({ color: tint, alpha: 0.06 });
  v.glow.circle(-px * 0.55, 0, px * 0.18).fill({ color: tint, alpha: 0.55 });
  // ×N representative badge (count-stack philosophy on screen).
  v.badge.text = v.reps > 1 ? `×${v.reps}` : "";
  v.badge.position.set(px * 0.4, -px * 0.35);
  // Titan nameplate (the flagship name lives on the syndicate, as shipped).
  const name = !v.plat && v.kind === "titan" ? st.rec.sides[v.side]?.flagship_name ?? "" : "";
  v.plate.text = name;
  v.plate.position.set(0, px * 0.6);
}

// --- The frame loop --------------------------------------------------------------------

function frame(dt: number): void {
  if (!st || !layers) return;
  sceneClock += dt;
  // Degradation ladder bookkeeping (rolling fps estimate).
  if (dt > 0) {
    fpsEma = fpsEma * 0.95 + (1 / Math.max(1e-3, dt)) * 0.05;
    perfTier = fpsEma >= 45 ? Math.max(0, perfTier - (fpsEma > 55 ? 1 : 0)) : Math.min(3, perfTier + 1);
  }
  rebuildWindow();
  const { frac, live } = st;
  const atFrontier = st.round >= st.rec.rounds.length - 1;
  const holdLive = live && atFrontier;
  for (const v of st.ships) {
    if (!v.inUse) continue;
    // Eased interpolation along the matched track.
    const e = frac * frac * (3 - 2 * frac); // smoothstep
    let x = v.x0 + (v.x1 - v.x0) * e;
    let y = v.y0 + (v.y1 - v.y0) * e;
    // LIGHT-LIVE hold: the newest keyframe breathes — engine flicker + slight
    // station-keeping — rather than freezing while waiting for light.
    if (holdLive) {
      const r = mulberry32(v.seed);
      const p1 = r() * Math.PI * 2;
      const p2 = r() * Math.PI * 2;
      x += Math.sin(sceneClock * 0.7 + p1) * 6;
      y += Math.cos(sceneClock * 0.55 + p2) * 6;
      v.glow.alpha = 0.8 + 0.2 * Math.sin(sceneClock * 5 + p1);
    } else {
      v.glow.alpha = 1;
    }
    v.root.position.set(sx(x), sy(y));
    // Smoothed turning toward the track heading (no snap).
    if (!v.plat) {
      const want = v.hdg;
      let cur = v.root.rotation - Math.PI / 2;
      let d = want - cur;
      while (d > Math.PI) d -= 2 * Math.PI;
      while (d < -Math.PI) d += 2 * Math.PI;
      cur += d * Math.min(1, dt * 6);
      v.root.rotation = cur + Math.PI / 2; // art is nose-up
    }
    // Labels stay horizontal whatever the hull is doing.
    v.badge.rotation = -v.root.rotation;
    v.plate.rotation = -v.root.rotation;
    // Wave arrivals fade in; a ship bound to a recorded DEATH disappears at
    // its exact moment (the explosion takes over); other departures (withdrew
    // or sampled out) fade.
    const idx = st.ships.indexOf(v);
    const death = st.fx?.deaths.find((d) => d.shipIdx === idx);
    let a = v.entered ? Math.min(1, frac * 3) : 1;
    if (death) a = frac >= death.t ? 0 : 1;
    else if (v.exiting) a = Math.max(0.15, 1 - frac);
    v.root.alpha = a * (0.45 + 0.55 * v.hp);
    // Withdrawal: engines flare hard while the side burns for the edge.
    if (!v.plat && st.round >= st.withdrawFrom[v.side]) {
      const px = spritePx(v.kind);
      v.glow.clear();
      const tint = st.rec.own_side !== null && v.side === st.rec.own_side ? TINT_OWN : TINT_FOE;
      v.glow.circle(0, 0, px * 0.5).fill({ color: tint, alpha: 0.06 });
      v.glow.circle(-px * 0.62, 0, px * 0.3).fill({ color: 0xffd98a, alpha: 0.75 });
      v.glow.circle(-px * 0.95, 0, px * 0.16).fill({ color: 0xffb46b, alpha: 0.5 });
    }
  }
  drawDebris();
  drawFx(dt);
  applyShakeAndBanner();
}

// --- FX rendering (immediate mode on persistent surfaces) --------------------------

const shipXY = (i: number, frac: number): [number, number] => {
  const v = st!.ships[i];
  if (!v) return [0, 0];
  const e = frac * frac * (3 - 2 * frac);
  return [v.x0 + (v.x1 - v.x0) * e, v.y0 + (v.y1 - v.y0) * e];
};

/// Quadratic bezier point for a torpedo arc (curving pursuit).
function arcPoint(a: FxWindow["arcs"][number], t: number, frac: number): [number, number] {
  const [tx, ty] = shipXY(a.to, frac);
  const u = 1 - t;
  return [u * u * a.x0 + 2 * u * t * a.cx + t * t * tx, u * u * a.y0 + 2 * u * t * a.cy + t * t * ty];
}

function drawDebris(): void {
  if (!debrisG || !st) return;
  debrisG.clear();
  const now = st.round + st.frac;
  for (const d of debrisField) {
    const age = Math.max(0, now - d.bornRound);
    const x = d.x + d.dx * age;
    const y = d.y + d.dy * age;
    if (Math.abs(x) > VIEW_R || Math.abs(y) > VIEW_R) continue;
    debrisG.circle(sx(x), sy(y), d.r).fill({ color: d.tint, alpha: d.tint === 0xff9d5c ? 0.5 : 0.35 });
  }
}

/// The weapon-FX grammar, drawn statelessly for the current (round, frac):
/// every family visually distinct, every effect scaled to the record.
function drawFx(dt: number): void {
  if (!fxG || !st) return;
  void dt;
  fxG.clear();
  const fx = st.fx;
  if (!fx) return;
  const f = st.frac;

  // BEAMS — instant flash-lines alive for a short window around t; heavy
  // (capital) beams show a charge-up glow then hold the line longer.
  for (const b of fx.beams) {
    const dur = b.heavy ? 0.14 : 0.06;
    const dtb = f - b.t;
    if (b.heavy && dtb > -0.05 && dtb < 0) {
      const [x, y] = shipXY(b.from, f);
      fxG.circle(sx(x), sy(y), 5 + 60 * (dtb + 0.05)).fill({ color: 0x9fd9ff, alpha: 0.35 });
      continue;
    }
    if (dtb < 0 || dtb > dur) continue;
    const k = 1 - dtb / dur;
    const [x0, y0] = shipXY(b.from, f);
    const [x1, y1] = shipXY(b.to, f);
    fxG.moveTo(sx(x0), sy(y0)).lineTo(sx(x1), sy(y1))
      .stroke({ color: 0x9fd9ff, width: (b.heavy ? 2.4 : 1) * b.w * k + 0.4, alpha: 0.55 + 0.4 * k });
    if (b.glint) {
      // Reflective mitigation: a mirror-flash deflection sparkle, not a bloom.
      fxG.moveTo(sx(x1) - 5, sy(y1)).lineTo(sx(x1) + 5, sy(y1)).stroke({ color: 0xffffff, width: 1, alpha: 0.9 * k });
      fxG.moveTo(sx(x1), sy(y1) - 5).lineTo(sx(x1), sy(y1) + 5).stroke({ color: 0xffffff, width: 1, alpha: 0.9 * k });
    } else {
      fxG.circle(sx(x1), sy(y1), 2.5 + 3.5 * b.w * k).fill({ color: 0xcfeaff, alpha: 0.5 * k });
    }
  }

  // DRIVER tracers — short visible flight with muzzle flash and impact
  // sparks; misses pass close and carry on (dropped first by the ladder).
  const FLIGHT = 0.1;
  for (const tr of fx.tracers) {
    if (tr.miss && perfTier >= 1) continue;
    const dtt = f - tr.t;
    if (dtt < 0 || dtt > FLIGHT + 0.06) continue;
    const [x0, y0] = shipXY(tr.from, f);
    const [x1raw, y1raw] = shipXY(tr.to, f);
    const missOff = tr.miss ? 26 : 0;
    const x1 = x1raw + missOff, y1 = y1raw + missOff * 0.6;
    const p = Math.min(1, dtt / FLIGHT);
    const q = Math.max(0, p - (perfTier >= 2 ? 0.1 : 0.16)); // ladder: thinner streak
    const ax = x0 + (x1 - x0) * (tr.miss ? p * 1.35 : p);
    const ay = y0 + (y1 - y0) * (tr.miss ? p * 1.35 : p);
    const bx = x0 + (x1 - x0) * (tr.miss ? q * 1.35 : q);
    const by = y0 + (y1 - y0) * (tr.miss ? q * 1.35 : q);
    fxG.moveTo(sx(bx), sy(by)).lineTo(sx(ax), sy(ay)).stroke({ color: 0xe8a13a, width: 1.1, alpha: 0.85 });
    if (dtt < 0.03) fxG.circle(sx(x0), sy(y0), 2.2).fill({ color: 0xffd98a, alpha: 0.8 }); // muzzle
    if (!tr.miss && p >= 1) {
      if (tr.spall) {
        // Whipple mitigation: shattered-armor spall puffs, not clean sparks.
        const rngS = mulberry32(hashId(`${st.rec.id}:${st.round}:spall:${tr.from}:${tr.to}`));
        for (let k = 0; k < 4; k++) {
          fxG.circle(sx(x1) + (rngS() - 0.5) * 12, sy(y1) + (rngS() - 0.5) * 12, 1.6).fill({ color: 0x9aa8ba, alpha: 0.7 });
        }
      } else {
        fxG.star(sx(x1), sy(y1), 4, 4, 1.4).fill({ color: 0xffe0a8, alpha: 0.85 });
      }
    }
  }

  // TORPEDO ARCS — the centerpiece: engine-glow trails curving toward their
  // targets; flak deaths burst short of the target; hits detonate biggest.
  for (const a of fx.arcs) {
    const prog = Math.min(f / a.tEnd, 1);
    if (f > a.tEnd + 0.12 && a.outcome !== "fly") continue;
    const endT = a.outcome === "flak" ? 0.72 : 1; // flak kills it short of the target
    const t = Math.min(prog, 1) * endT;
    const [hx, hy] = arcPoint(a, t, f);
    if (f <= a.tEnd) {
      for (let k = 0; k < 4; k++) { // trail
        const [px, py] = arcPoint(a, Math.max(0, t - 0.05 * (k + 1)), f);
        fxG.circle(sx(px), sy(py), 1.5 - k * 0.28).fill({ color: 0xe0574b, alpha: 0.6 - k * 0.13 });
      }
      fxG.circle(sx(hx), sy(hy), 1.8).fill({ color: 0xffb0a0, alpha: 0.95 });
    }
    const since = f - a.tEnd;
    if (since >= 0 && since < 0.12 && a.outcome === "flak") {
      fxG.circle(sx(hx), sy(hy), 3 + 26 * since).stroke({ color: 0xffd98a, width: 1.2, alpha: 0.8 * (1 - since / 0.12) });
    }
    if (since >= 0 && since < 0.12 && a.outcome === "hit") {
      const [tx2, ty2] = shipXY(a.to, f);
      fxG.circle(sx(tx2), sy(ty2), 4 + 46 * since).fill({ color: 0xffb46b, alpha: 0.55 * (1 - since / 0.12) });
      fxG.circle(sx(tx2), sy(ty2), 2 + 20 * since).fill({ color: 0xfff2cc, alpha: 0.8 * (1 - since / 0.12) });
    }
  }

  // POINT-DEFENSE fans — screening ships spray rapid small tracers at arcs
  // inside their bubble (a Dreadnought's fan is denser and longer-ranged).
  for (const pi of fx.pdShips) {
    const v = st.ships[pi];
    if (!v?.inUse) continue;
    const [px, py] = shipXY(pi, f);
    const radius = v.kind === "dreadnought" ? 400 : 180;
    const dense = v.kind === "dreadnought" ? 5 : 3;
    for (const a of fx.arcs) {
      if (a.side === v.side) continue;
      if (f > a.tEnd) continue;
      const t = Math.min(f / a.tEnd, 1) * (a.outcome === "flak" ? 0.72 : 1);
      const [hx, hy] = arcPoint(a, t, f);
      if (Math.hypot(hx - px, hy - py) > radius) continue;
      const rngF = mulberry32(hashId(`${st.rec.id}:${st.round}:pd:${pi}`) ^ Math.floor(f * 24));
      for (let k = 0; k < dense; k++) {
        const jx = hx + (rngF() - 0.5) * 30, jy = hy + (rngF() - 0.5) * 30;
        fxG.moveTo(sx(px), sy(py)).lineTo(sx(jx), sy(jy)).stroke({ color: 0xbfe0ff, width: 0.5, alpha: 0.4 });
      }
    }
  }

  // DEATHS — exact record events: explosion scaled by mass class. Corvettes
  // pop; cruisers flash-and-break; capitals go in multi-stage breakups.
  for (const d of fx.deaths) {
    const since = f - d.t;
    if (since < 0) continue;
    const life = d.cls === 3 ? 0.5 : d.cls === 2 ? 0.3 : 0.16;
    if (since > life) continue;
    const k = since / life;
    const base = [10, 16, 26, 40][d.cls];
    fxG.circle(sx(d.x), sy(d.y), 2 + base * k).fill({ color: 0xffb46b, alpha: 0.5 * (1 - k) });
    fxG.circle(sx(d.x), sy(d.y), 1 + base * 0.45 * k).fill({ color: 0xfff2cc, alpha: 0.85 * (1 - k) });
    if (d.cls >= 2) {
      // Multi-stage: burning sections shed outward on a seeded fan.
      const rngD = mulberry32(hashId(`${st.rec.id}:death:${d.x.toFixed(0)}:${d.y.toFixed(0)}`));
      for (let s2 = 0; s2 < (d.cls === 3 ? 7 : 4); s2++) {
        const ang = rngD() * Math.PI * 2;
        const rr = (14 + rngD() * 30) * k;
        fxG.circle(sx(d.x) + Math.cos(ang) * rr, sy(d.y) + Math.sin(ang) * rr, 2.4 * (1 - k) + 0.6)
          .fill({ color: 0xff9d5c, alpha: 0.7 * (1 - k) });
      }
    }
  }
}

/// Titan set piece: screen shake within taste + the flagship-name banner
/// (mirrors the sim's news headline). Shake decays; scrub-safe (pure f(t)).
function applyShakeAndBanner(): void {
  if (!st || !app || !layers) return;
  const f = st.frac;
  let shake = 0;
  for (const d of st.fx?.deaths ?? []) {
    if (d.cls < 2) continue;
    const since = f - d.t;
    if (since >= 0 && since < 0.5) shake = Math.max(shake, (d.cls === 3 ? 5 : 2.4) * (1 - since / 0.5));
  }
  const rngS = mulberry32(hashId(`${st.rec.id}:${st.round}:shake`) ^ Math.floor(f * 60));
  app.stage.position.set(shake ? (rngS() - 0.5) * 2 * shake : 0, shake ? (rngS() - 0.5) * 2 * shake : 0);
  // Banner: the Titan death headline, once per (record, round).
  const titan = st.fx?.deaths.find((d) => d.cls === 3);
  if (banner) {
    if (titan && f >= titan.t) {
      const key = `${st.rec.id}:${st.round}`;
      const name = st.rec.sides[titan.side]?.flagship_name ?? "THE TITAN";
      if (bannerKey !== key) { bannerKey = key; banner.textContent = `☄ ${name.toUpperCase()} IS DOWN`; }
      banner.style.display = "block";
      banner.style.opacity = String(Math.max(0, 1 - Math.max(0, f - titan.t - 0.35) * 2));
    } else if (bannerKey && !titan) {
      banner.style.display = "none";
    } else if (!titan || f < titan.t) {
      banner.style.display = "none";
    }
  }
}

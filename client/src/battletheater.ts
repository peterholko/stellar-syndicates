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

interface TheaterState {
  rec: BattleRecordView;
  round: number; // active window = frames[round] → frames[round+1]
  frac: number;
  live: boolean;
  ships: ShipVis[];
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
  if (st && st.rec.id === rec.id) st.rec = rec;
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
export function theaterDebug(): { ships: number; fx: number; ticker: boolean; round: number; frac: number } | null {
  if (!st || !layers || !app) return null;
  return {
    ships: st.ships.filter((s) => s.inUse).length,
    fx: layers.fx.children.length,
    ticker: app.ticker.started,
    round: st.round,
    frac: st.frac,
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
    st.rec = rec;
    return;
  }
  st = {
    rec,
    round: 0,
    frac: 0,
    live: false,
    ships: [],
    windowKey: "",
  };
  sceneClock = 0;
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
    // Fade for wave arrivals / departures (deaths get real FX in §TH2).
    const a = v.entered ? Math.min(1, frac * 3) : v.exiting ? Math.max(0.15, 1 - frac) : 1;
    v.root.alpha = a * (0.45 + 0.55 * v.hp);
  }
  drawTorpedoClusters();
}

/// Minimal torpedo presence for the core stage: the per-side salvo cluster at
/// its interpolated centroid (the full individual-arc grammar lands in §TH2).
function drawTorpedoClusters(): void {
  if (!st || !layers) return;
  layers.fx.removeChildren().forEach((c) => c.destroy());
  const f0 = st.rec.rounds[st.round]?.frame;
  const f1 = st.rec.rounds[st.round + 1]?.frame ?? f0;
  if (!f0) return;
  const g = new Graphics();
  for (let side = 0; side < 2; side++) {
    const s0 = f0.torpedoes.find((t) => t.side === side);
    const s1 = f1?.torpedoes.find((t) => t.side === side) ?? s0;
    if (!s0 || s0.n === 0) continue;
    const e = st.frac;
    const cx = s0.x + ((s1?.x ?? s0.x) - s0.x) * e;
    const cy = s0.y + ((s1?.y ?? s0.y) - s0.y) * e;
    const rng = mulberry32(hashId(`${st.rec.id}:${st.round}:torp:${side}`));
    const shown = Math.min(12, s0.n);
    for (let i = 0; i < shown; i++) {
      const ox = (rng() - 0.5) * 60;
      const oy = (rng() - 0.5) * 60;
      g.circle(sx(cx + ox), sy(cy + oy), 1.6).fill({ color: 0xe0574b, alpha: 0.9 });
    }
  }
  layers.fx.addChild(g);
}

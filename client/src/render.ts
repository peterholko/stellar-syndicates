// Pixi.js renderer. Draws the player's DELAYED, FOGGED view (§6) — the heart of
// the game made visible (Pillar 2: never hide the lag). Each ship is a ghost at
// the position its arriving light shows; EVERY ghost — own or enemy — carries an
// uncertainty cone (how far it could have moved since the light left) and an age
// label, and fades with staleness. There is no FTL tether to your own fleet:
// certainty comes from PROXIMITY to the command center, so a distant own ship is
// just as fogged as a distant enemy, while one nearby is crisp. An own ship under
// orders also shows a hint of where it has most likely advanced along its course.
// The command center is your vantage — the origin of everything you can see.

import { Application, Assets, Container, Graphics, Sprite, Text, TextStyle, Texture } from "pixi.js";
import type { Commodity, GalaxyInfo, GhostView, ShipKind, SystemInfo, Vec2 } from "./protocol";
import { countClassLabel, fleetExactCount } from "./protocol";
import type { ViewState } from "./state";
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
export type MapViewMode = { type: "galaxy" } | { type: "system"; systemId: string };

// Crossfade + camera-push transition between the two scenes.
const TRANS_MS = 480;
const easeInOut = (t: number): number => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2);
const clamp01 = (x: number): number => Math.max(0, Math.min(1, x));
interface Transition {
  dir: "in" | "out";
  start: number;
  camFrom: { cx: number; cy: number; scale: number };
  camTo: { cx: number; cy: number; scale: number };
}

const COL_HUB = 0x7fd4ff;
const COL_SYSTEM = 0x4a5d7a;

// Mirror of the sim's base prices: ranks how valuable a deposit is, for sizing
// the frontier-richer glow and picking a system's dominant-resource tint.
const COMMODITY_VALUE: Record<Commodity, number> = {
  provisions: 6,
  ore: 8,
  fuel: 10,
  volatiles: 18,
  alloys: 26,
};
const COMMODITY_COLOR: Record<Commodity, number> = {
  provisions: 0x7fdc8a,
  ore: 0xb0894f,
  fuel: 0xff9d5c,
  volatiles: 0x6bd0ff,
  alloys: 0xc99bff,
};
const COL_OWN = 0x4fc3ff;
const COL_OTHER = 0xff7a6b;
const COL_ANCHOR_OWN = 0x9be7ff;
const COL_ANCHOR_OTHER = 0xcf9b6b;
const COL_CONE = 0xff7a6b;
const COL_COMMAND = 0xc56bff; // outbound order comet (violet)
const COL_REPORT = 0xffd24a; // known convoy cargo label (gold = intel)
const COL_SENSOR = 0x3fe0c8; // sensor coverage (teal)
const COL_THREAT = 0xff4d4d; // detected raider (alert red)
const COL_ESTIMATE = 0xffae5c; // crude intercept estimate (soft amber, fuzzy)
// Ships render in their NATURAL art — no per-syndicate body tint (a future
// ownership indicator is TBD). This neutral is only the primitive fallback hull
// shown before the sprite art loads; it must NOT imply ownership.
const COL_SHIP_NEUTRAL = 0xc9d6e8;

const MAX_EXTRAPOLATE_S = 0.4;
const FADE_AGE_S = 45; // staleness at which an enemy ghost is most faded

// --- Zoom limits, as multiples of the fit-to-galaxy scale (so they scale with
// galaxy size). MIN ≈ fit (whole galaxy visible, a touch looser); MAX inspects a
// single system / tight cluster for precise clicking. ---
const ZOOM_MIN_FACTOR = 0.9;
const ZOOM_MAX_FACTOR = 24;

interface GhostSprite {
  container: Container;
  cone: Graphics;
  body: Graphics; // primitive triangle — fallback until the ship sprite loads
  sprite: Sprite; // the ship art (rotated to heading, tinted by ownership)
  label: Text;
  ring: Graphics; // selection ring
  pip: Graphics; // ownership tag (cyan = yours, red = rival) — the friend/foe cue
  badge: Graphics; // fleet count pill (exact Σ, or the fog size bucket)
  badgeText: Text; // the number / bucket label drawn on the badge
  seen: boolean;
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
const SHIP_ZOOM_MAX = 1.6; // indicator growth cap (normal-zoom phase)
// Deep-zoom NATIVE-size ramp: the zoom ratio r (= scale / fitScale) at which
// ships BEGIN ramping from their indicator size (base × SHIP_ZOOM_MAX) up to
// their TRUE NATIVE texture size, reaching native exactly at ZOOM_MAX_FACTOR.
// Below this, ships stay small map indicators exactly as before. ~12 is the top
// half of the 0.9→24 zoom range. Tunable — set it near ZOOM_MAX_FACTOR for a
// last-sliver "snap," or lower for an earlier, gentler ramp.
const SHIP_NATIVE_ZOOM_START = 12;

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
  private sensorGfx = new Graphics();
  private routesGfx = new Graphics();
  private systemsLayer = new Container();
  private anchorsLayer = new Container();
  private orderLayer = new Container();
  private interceptGfx = new Graphics(); // soft intercept-estimate zones
  private ghostsLayer = new Container();
  private signalsLayer = new Container();
  private signalsGfx = new Graphics();
  private interceptLabels = new Map<string, Text>();
  private ghosts = new Map<string, GhostSprite>();

  // Celestial body sprites (planet = star system, station = hub), pooled in a
  // persistent layer UNDER the ownership/value/label cues so those still read.
  private bodyLayer = new Container();
  private systemBodies = new Map<string, Sprite>();
  private hubSprite: Sprite | null = null;
  // Star-type map icons, keyed by slug — a system draws the icon for its
  // deterministically-assigned type (stars.ts). Loaded lazily in loadArt.
  private starTex = new Map<string, Texture>();
  private texStation: Texture | null = null;
  // Ship sprites (convoy = freighter, raider = attack ship), top-down (nose = -y).
  private texConvoy: Texture | null = null;
  private texRaider: Texture | null = null;
  private texCorvette: Texture | null = null;
  private texColony: Texture | null = null;
  private texScout: Texture | null = null;

  // The schematic System View scene (its own camera). Presentation only.
  private systemScene = new SystemViewScene();
  private mode: MapViewMode = { type: "galaxy" };
  private transition: Transition | null = null;

  private galaxy: GalaxyInfo | null = null;
  private scale = 1;
  private cx = 0;
  private cy = 0;
  /// True once the user has zoomed/panned — so a window resize PRESERVES their
  /// view (re-clamping scale) instead of snapping back to fit-to-galaxy.
  private userView = false;
  /// The world-anchored background (galaxy rings + hub) is drawn only when the
  /// transform changes, not every frame; this flags it for redraw.
  private viewDirty = false;

  async init(mount: HTMLElement): Promise<void> {
    await this.app.init({
      background: "#05070d",
      resizeTo: window,
      antialias: true,
      autoDensity: true,
      resolution: window.devicePixelRatio || 1,
    });
    mount.appendChild(this.app.canvas);
    // Galaxy scene: all existing gameplay layers under ONE root so it can be
    // faded/pushed as a unit during the semantic-zoom transition. Draw order and
    // the per-layer camera math are unchanged — only the parent is now galaxyRoot.
    this.galaxyRoot.addChild(
      this.bg,
      this.sensorGfx, // soft sensor coverage, under everything gameplay
      this.bodyLayer, // celestial body sprites, under the data cues that decorate them
      this.systemsLayer,
      this.anchorsLayer,
      this.routesGfx, // convoy broadcast routes, under ghosts
      this.orderLayer,
      this.interceptGfx, // soft intercept estimate, under the ghosts it guides
      this.ghostsLayer,
      this.signalsLayer,
    );
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
    const [station, convoy, raider, corvette, colony, scout] = await Promise.all([
      load("/art/celestial_sprites/mining_station.png"),
      load("/art/ship_sprites/cargo_freighter.png"),
      load("/art/ship_sprites/raider_attack_ship.png"),
      load("/art/ship_sprites/corvette_escort_ship.png"),
      load("/art/ship_sprites/colony_ship.png"),
      load("/art/ship_sprites/scout_utility_ship.png"),
    ]);
    this.texStation = station;
    this.texConvoy = convoy;
    this.texRaider = raider;
    this.texCorvette = corvette;
    this.texColony = colony;
    this.texScout = scout;
    // The star-type icons (each independent; a missing one falls back to the dot).
    await Promise.all(
      STAR_TYPES.map(async (t) => {
        const tex = await load(starIconUrl(t));
        if (tex) this.starTex.set(t.slug, tex);
      }),
    );
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
    if (!this.galaxy) return;
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
    this.cx += dx;
    this.cy += dy;
    this.userView = true;
    this.viewDirty = true;
  }
  /// Reset to the fit-to-galaxy view (and let subsequent resizes re-fit again).
  resetView(): void {
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
  /// Camera to restore when leaving the System View (the player's pre-enter view).
  private savedGalaxyCam: { cx: number; cy: number; scale: number } | null = null;

  /// ENTER the schematic System View for a system: build its (deterministic,
  /// public) visual schematic, save the current galaxy camera, and start the
  /// crossfade + camera-push toward the star. No sim/protocol change — the scene
  /// renders only public geography + the light-gated ownership already in `state`.
  enterSystemView(sys: SystemInfo): void {
    if (this.mode.type === "system" && this.mode.systemId === sys.id) return;
    const st = starTypeFor(sys.id);
    this.systemScene.setSystem(buildVisualSystem(sys), this.starTex.get(st.slug) ?? null);
    this.systemScene.layout(this.viewW, this.viewH);
    this.savedGalaxyCam = { cx: this.cx, cy: this.cy, scale: this.scale };
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    // Push the galaxy camera to center the star at max zoom, so the map visibly
    // dives toward it as the schematic fades in (an LOD change that FEELS
    // connected — not a literal zoom through astronomical space).
    const toScale = this.fitScale() * ZOOM_MAX_FACTOR;
    const camTo = { cx: this.viewW / 2 - sys.pos.x * toScale, cy: this.viewH / 2 - sys.pos.y * toScale, scale: toScale };
    this.systemScene.root.visible = true;
    this.systemScene.root.alpha = 0;
    this.mode = { type: "system", systemId: sys.id };
    this.userView = true;
    this.transition = { dir: "in", start: performance.now(), camFrom, camTo };
  }

  /// EXIT back to the galaxy, restoring the pre-enter camera as the schematic
  /// crossfades out and the galaxy pulls back.
  exitSystemView(): void {
    if (this.mode.type !== "system") return;
    const restore = this.savedGalaxyCam ?? { cx: this.viewW / 2, cy: this.viewH / 2, scale: this.fitScale() };
    const camFrom = { cx: this.cx, cy: this.cy, scale: this.scale };
    this.systemScene.clearSelection();
    this.mode = { type: "galaxy" };
    this.transition = { dir: "out", start: performance.now(), camFrom, camTo: restore };
  }

  /// Hit-test a planet/moon in the System View (opens a details panel — the ONLY
  /// planet interaction; no per-planet gameplay, no deeper camera level).
  systemPick(sx: number, sy: number): SystemBodyDetail | null {
    return this.systemScene.pickBody(sx, sy);
  }

  setGalaxy(galaxy: GalaxyInfo): void {
    this.galaxy = galaxy;
    // Drop pooled body sprites from any previous galaxy (fresh systems / ids) —
    // the per-system bodies AND the hub, so a galaxy change leaves no stale sprite.
    for (const sp of this.systemBodies.values()) sp.destroy();
    this.systemBodies.clear();
    this.hubSprite?.destroy();
    this.hubSprite = null;
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

  private drawBackground(): void {
    this.bg.removeChildren();
    if (!this.galaxy) return;
    const g = new Graphics();
    const rPx = this.galaxy.radius * this.scale;
    g.circle(this.cx, this.cy, rPx).stroke({ width: 1, color: 0x1c2740, alpha: 0.9 });
    for (const f of [0.33, 0.66]) {
      g.circle(this.cx, this.cy, rPx * f).stroke({ width: 1, color: 0x141d30, alpha: 0.8 });
    }
    const hub = this.worldToScreen(this.galaxy.hub);
    g.circle(hub.x, hub.y, 11).fill({ color: COL_HUB, alpha: 0.18 });
    g.circle(hub.x, hub.y, 6).fill({ color: COL_HUB, alpha: 0.4 });
    g.circle(hub.x, hub.y, 2.5).fill({ color: 0xffffff, alpha: 0.9 });
    this.bg.addChild(g);
    const label = new Text({ text: "HUB", style: new TextStyle({ fill: COL_HUB, fontFamily: "ui-monospace, monospace", fontSize: 10, letterSpacing: 2 }) });
    label.anchor.set(0.5, 0);
    label.position.set(hub.x, hub.y + 13);
    this.bg.addChild(label);
  }

  /// Draw star systems with their resource geology and (light-gated) ownership.
  /// A system's glow grows with its deposit value-rate, so the frontier visibly
  /// out-produces the core (§4); the ring shows ownership — cyan (yours), red (a
  /// rival, once their claim's light has reached you), or dim (unclaimed). Your
  /// own systems also surface their accumulated production.
  private drawSystems(state: ViewState): void {
    this.systemsLayer.removeChildren();
    if (!this.galaxy) return;
    const dynById = new Map(state.systems.map((s) => [s.id, s]));
    for (const sys of this.galaxy.systems) {
      const s = this.worldToScreen(sys.pos);
      const dyn = dynById.get(sys.id);
      const owner = dyn?.owner ?? null;
      const mine = owner !== null && owner === state.playerId;
      const rival = owner !== null && !mine;
      const selected = state.selectedSystemId === sys.id;

      // Value-rate → glow size; dominant resource → tint (the gradient made visible).
      let valueRate = 0;
      let topVal = -1;
      let topColor = COL_SYSTEM;
      for (const d of sys.deposits) {
        const v = d.richness * (COMMODITY_VALUE[d.resource] ?? 1);
        valueRate += v;
        if (v > topVal) {
          topVal = v;
          topColor = COMMODITY_COLOR[d.resource] ?? COL_SYSTEM;
        }
      }
      const glow = Math.min(3 + valueRate * 0.45, 18);

      const g = new Graphics();
      g.circle(s.x, s.y, glow).fill({ color: topColor, alpha: 0.07 }); // geology value-glow

      // Ownership treatment — own and rival are a matched pair (halo + bold ring),
      // so territory reads at a glance; unclaimed systems stay deliberately subdued
      // (no ring) so they recede. Ownership is still light-gated upstream: a rival
      // only appears as rival once their claim's light has reached this player.
      if (mine) {
        // Friendly territory: cyan halo + bold ring.
        g.circle(s.x, s.y, 10).fill({ color: COL_OWN, alpha: 0.10 });
        g.circle(s.x, s.y, 7).stroke({ width: 1.8, color: COL_OWN, alpha: 0.95 });
      } else if (rival) {
        // Rival / contested territory: a slow-breathing red danger halo + a bold
        // DOUBLE ring — unmistakable as hostile-held, and clearly distinct from the
        // fast-pulsing raider-threat marker (slower cadence, static rings, sized to
        // the system body, softer COL_OTHER hue vs. the alert COL_THREAT red).
        const breath = 0.5 + 0.5 * Math.sin(performance.now() / 1100);
        g.circle(s.x, s.y, 13).fill({ color: COL_OTHER, alpha: 0.05 + 0.07 * breath });
        g.circle(s.x, s.y, 9.5).stroke({ width: 1, color: COL_OTHER, alpha: 0.4 });
        g.circle(s.x, s.y, 7).stroke({ width: 2, color: COL_OTHER, alpha: 0.98 });
      }
      if (selected) {
        g.circle(s.x, s.y, owner !== null ? 12 : glow + 4).stroke({ width: 1.2, color: 0xffffff, alpha: 0.85 });
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
        const bodyD = Math.min(20 + valueRate * 0.9, 46); // target VISIBLE diameter
        const anchor = starAnchor(st);
        bsp.anchor.set(anchor[0], anchor[1]);
        bsp.position.set(s.x, s.y);
        bsp.scale.set(bodyD / (starVisualRatio(st) * starTex.width));
        // Keep unclaimed stars near-full brightness so the vivid star art reads
        // (ownership is carried by the RING, not by dimming the star); owned/rival
        // still lead via their full brightness + ring.
        bsp.alpha = owner !== null ? 1 : 0.9;
      } else {
        const dotCol = mine ? COL_OWN : rival ? COL_OTHER : COL_SYSTEM;
        g.circle(s.x, s.y, 2.4).fill({ color: dotCol, alpha: 0.95 });
      }
      this.systemsLayer.addChild(g);

      // Label: name; your own systems also show their top stockpiled good.
      let txt = sys.name;
      if (mine && dyn?.stockpile && dyn.stockpile.length) {
        const top = dyn.stockpile.reduce((a, b) => (a.units > b.units ? a : b));
        txt = `${sys.name}  ◆${top.units} ${top.commodity}`;
      }
      const col = mine ? COL_OWN : rival ? COL_OTHER : 0x55657f;
      const t = new Text({ text: txt, style: new TextStyle({ fill: col, fontFamily: "ui-monospace, monospace", fontSize: 8 }) });
      t.anchor.set(0, 0.5);
      t.position.set(s.x + glow + 2, s.y);
      t.alpha = mine ? 0.95 : rival ? 0.88 : selected ? 0.8 : 0.5;
      this.systemsLayer.addChild(t);
    }
  }

  private drawAnchors(state: ViewState): void {
    this.anchorsLayer.removeChildren();
    if (!this.galaxy) return;
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
        const g = new Graphics();
        const color = own ? COL_ANCHOR_OWN : COL_ANCHOR_OTHER;
        if (a.owner) {
          g.circle(s.x, s.y, own ? 9 : 6).fill({ color, alpha: own ? 0.22 : 0.14 });
          g.circle(s.x, s.y, 3).fill({ color, alpha: 0.9 });
        } else {
          g.circle(s.x, s.y, 4).stroke({ width: 1, color: 0x3a4660, alpha: 0.7 });
        }
        this.anchorsLayer.addChild(g);
      }
      // Name your own command seat "HOME" (above the home system's own label).
      if (own) {
        const t = new Text({ text: "HOME", style: new TextStyle({ fill: COL_ANCHOR_OWN, fontFamily: "ui-monospace, monospace", fontSize: 10, fontWeight: "700", letterSpacing: 2 }) });
        t.anchor.set(0.5, 1);
        t.position.set(s.x, s.y - 13);
        this.anchorsLayer.addChild(t);
      }
    }
  }

  /// Soft, fuzzy INTERCEPT ESTIMATES for committed raids (§8, §14.1). A CRUDE
  /// constant-velocity lead projection from the delayed ghosts — honest, since
  /// the real pursuit acts on light-delayed sightings it hasn't seen yet, so it
  /// is EXPECTED to drift. Rendered in the sensor-circle idiom
  /// (translucent, soft, concentric) precisely so it reads as "best guess, about
  /// here," the way a sensor circle reads as a soft boundary — honest uncertainty,
  /// not a precise promise.
  private drawIntercepts(state: ViewState): void {
    const g = this.interceptGfx;
    g.clear();
    const live = new Set<string>();
    if (this.galaxy) {
      const raiderSpeed = Math.max(this.galaxy.raider_speed || 100, 1);
      for (const [raiderId, targetId] of Object.entries(state.raids)) {
        const r = state.ghosts.find((x) => x.id === raiderId);
        const t = state.ghosts.find((x) => x.id === targetId);
        if (!r || !t) continue; // a ship left the view — no guess to draw

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
    for (const [id, label] of this.interceptLabels) {
      if (!live.has(id)) label.visible = false;
    }
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

  /// The command center: the player's vantage, with a pulsing ring.
  private drawCommandCenter(state: ViewState): void {
    if (!state.commandCenter) return;
    const s = this.worldToScreen(state.commandCenter);
    const g = new Graphics();
    const pulse = 0.5 + 0.5 * Math.sin(performance.now() / 600);
    g.circle(s.x, s.y, 14 + pulse * 4).stroke({ width: 1, color: COL_OWN, alpha: 0.25 + 0.25 * pulse });
    g.circle(s.x, s.y, 5).stroke({ width: 1.5, color: COL_OWN, alpha: 0.9 });
    this.anchorsLayer.addChild(g);
  }

  /// Sensor coverage: a soft bubble around each of the player's assets — their
  /// own ships + command center at the global sensor range, plus any OWNED
  /// SENSOR-ARRAY systems at their per-tier radius (§buildings step 2b; the
  /// same coverage union the server computes — one source of truth). The union
  /// shows where the player can detect raiders and read cargo — and, by what it
  /// doesn't cover, where they are blind. Owner-only by construction: array
  /// tiers come from the light-gated View, which reports 0 for rival systems.
  private drawSensorCoverage(state: ViewState, dt: number): void {
    const g = this.sensorGfx;
    g.clear();
    if (!state.galaxy || !state.commandCenter) return;
    const baseR = state.galaxy.sensor_range;
    const sources: { x: number; y: number; r: number }[] = [{ ...state.commandCenter, r: baseR }];
    for (const gh of state.ghosts) {
      // Each own ship projects its KIND's bubble — a scout an oversized one
      // (scout_sensor_mult; mobile vision, mirroring the server's coverage).
      const r = gh.kind === "scout" ? baseR * (state.galaxy.scout_sensor_mult ?? 1.5) : baseR;
      if (gh.own) sources.push({ x: gh.pos.x + gh.vel.x * dt, y: gh.pos.y + gh.vel.y * dt, r });
    }
    // Standing array bubbles at OUR systems (sensor_tier is owner-only in the View).
    for (const dyn of state.systems) {
      if (dyn.owner === state.playerId && dyn.sensor_tier >= 1) {
        const sys = state.galaxy.systems.find((s) => s.id === dyn.id);
        if (sys) {
          const r = state.galaxy.sensor_array_base + state.galaxy.sensor_array_per_tier * (dyn.sensor_tier - 1);
          sources.push({ x: sys.pos.x, y: sys.pos.y, r });
        }
      }
    }
    for (const c of sources) {
      const s = this.worldToScreen(c);
      const rPx = c.r * this.scale;
      g.circle(s.x, s.y, rPx).fill({ color: COL_SENSOR, alpha: 0.045 }).stroke({ width: 1, color: COL_SENSOR, alpha: 0.14 });
    }

    // DEFENSE PLATFORM protection rings on OUR OWN defended systems (§buildings
    // step 2c) — owner-only by construction (defense_tier is 0 for rivals in the
    // View). Drawn in the coverage idiom but visually DISTINCT from the teal
    // sensor bubbles: a dashed cyan ring, no fill — "protected zone", not vision.
    for (const dyn of state.systems) {
      if (dyn.owner === state.playerId && dyn.defense_tier >= 1) {
        const sys = state.galaxy.systems.find((s) => s.id === dyn.id);
        if (sys) {
          const s = this.worldToScreen(sys.pos);
          const rPx = state.galaxy.defense_platform_radius * this.scale;
          dashedCircle(g, s.x, s.y, rPx, 10, 8);
          g.stroke({ width: 1.2, color: COL_OWN, alpha: 0.22 });
        }
      }
    }
  }

  /// Convoy broadcast routes: because convoys broadcast position + heading, show
  /// their waypoints and the path between them (light-delayed like the rest).
  private drawRoutes(state: ViewState): void {
    const g = this.routesGfx;
    g.clear();
    for (const gh of state.ghosts) {
      if (gh.kind !== "convoy" || !gh.route || gh.route.length < 1) continue;
      const color = gh.own ? COL_OWN : COL_OTHER;
      const pts = gh.route.map((w) => this.worldToScreen(w));
      g.moveTo(pts[0].x, pts[0].y);
      for (let i = 1; i < pts.length; i++) g.lineTo(pts[i].x, pts[i].y);
      if (pts.length > 2) g.lineTo(pts[0].x, pts[0].y); // close the patrol loop
      g.stroke({ width: 1, color, alpha: 0.2 });
      for (const p of pts) g.circle(p.x, p.y, 2.4).stroke({ width: 1, color, alpha: 0.45 });
    }
  }

  private drawOrders(state: ViewState, ghostById: Map<string, { x: number; y: number }>): void {
    this.orderLayer.removeChildren();
    for (const [shipId, dest] of Object.entries(state.orders)) {
      const from = ghostById.get(shipId);
      if (!from) continue;
      const to = this.worldToScreen(dest);
      const g = new Graphics();
      // Dashed line from the ghost to its commanded destination.
      dashedLine(g, from.x, from.y, to.x, to.y, 6, 5);
      g.stroke({ width: 1, color: COL_OWN, alpha: 0.45 });
      g.circle(to.x, to.y, 3).stroke({ width: 1, color: COL_OWN, alpha: 0.7 });
      this.orderLayer.addChild(g);
    }
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

  /// The hub body: a station sprite at the wormhole hub (over its teal glow, which
  /// stays in the background). Positioned each frame so it tracks zoom/pan.
  private drawHubBody(): void {
    if (!this.galaxy || !this.texStation) return;
    if (!this.hubSprite) {
      this.hubSprite = new Sprite(this.texStation);
      this.hubSprite.anchor.set(0.5);
      this.bodyLayer.addChild(this.hubSprite);
    }
    const h = this.worldToScreen(this.galaxy.hub);
    this.hubSprite.position.set(h.x, h.y);
    this.hubSprite.scale.set(28 / this.texStation.width);
  }

  /// The ship art for a kind (null until loaded — primitive fallback covers it).
  private texFor(kind: ShipKind): Texture | null {
    switch (kind) {
      case "convoy": return this.texConvoy;
      case "raider": return this.texRaider;
      case "corvette": return this.texCorvette;
      case "colony": return this.texColony;
      case "scout": return this.texScout;
    }
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
      const label = new Text({ text: "", style: new TextStyle({ fill: COL_OTHER, fontFamily: "ui-monospace, monospace", fontSize: 9 }) });
      label.anchor.set(0, 0.5);
      const pip = new Graphics();
      // Fleet count badge: a small pill at the sprite's lower-right showing the
      // fleet size (exact when known, the fog bucket otherwise).
      const badge = new Graphics();
      const badgeText = new Text({ text: "", style: new TextStyle({ fill: 0xffffff, fontFamily: "ui-monospace, monospace", fontSize: 9, fontWeight: "bold" }) });
      badgeText.anchor.set(0.5, 0.5);
      // Pip is topmost so the friend/foe tag is never hidden by the sprite/label.
      container.addChild(cone, ring, body, sprite, label, badge, badgeText, pip);
      this.ghostsLayer.addChild(container);
      sp = { container, cone, body, sprite, label, ring, pip, badge, badgeText, seen: true };
      this.ghosts.set(id, sp);
    }
    return sp;
  }

  /// On-screen ship size (px) as a function of the current zoom, in TWO phases:
  ///  1. Normal / indicator: base × clamp(r, SHIP_ZOOM_MIN, SHIP_ZOOM_MAX) — the
  ///     small map markers, unchanged, across the whole normal zoom range.
  ///  2. Deep-zoom (r ≥ SHIP_NATIVE_ZOOM_START): smoothly ramp from the indicator
  ///     size UP TO the TRUE NATIVE texture width, reaching native exactly at
  ///     ZOOM_MAX_FACTOR. Seamless at the threshold — both phases give
  ///     base × SHIP_ZOOM_MAX there, so there's no pop — and it never exceeds
  ///     nativeW, so the sprite scale is always ≤ 1.0 (downscale-or-native, crisp).
  /// Both kinds converge to the SAME native size at max zoom: up close the art's
  /// SHAPE distinguishes convoy vs raider, so identical native size is intended.
  private shipSizePx(kind: ShipKind, nativeW: number): number {
    const base = kind === "convoy" ? SHIP_PX_CONVOY : kind === "raider" ? SHIP_PX_RAIDER : kind === "corvette" ? SHIP_PX_CORVETTE : kind === "colony" ? SHIP_PX_COLONY : SHIP_PX_SCOUT;
    const r = this.scale / this.fitScale();
    const indicator = base * Math.max(SHIP_ZOOM_MIN, Math.min(SHIP_ZOOM_MAX, r));
    if (r <= SHIP_NATIVE_ZOOM_START) return indicator;
    const t = Math.min((r - SHIP_NATIVE_ZOOM_START) / (ZOOM_MAX_FACTOR - SHIP_NATIVE_ZOOM_START), 1);
    const s = t * t * (3 - 2 * t); // smoothstep — gentle growth, not linear
    const from = base * SHIP_ZOOM_MAX; // indicator size at the handoff (seamless)
    return from + (nativeW - from) * s;
  }

  /// Half the ship's CURRENT on-screen size — the click hit radius, so ships stay
  /// clickable as they enlarge in the deep-zoom band. Falls back to a 256px native
  /// assumption before the texture loads. Consumed by main.ts's map hit-test.
  shipHitRadius(kind: ShipKind): number {
    const tex = this.texFor(kind);
    return this.shipSizePx(kind, tex ? tex.width : 256) / 2;
  }

  private drawGhost(ghost: GhostView, state: ViewState, dt: number): { x: number; y: number } {
    const sp = this.ghostSprite(ghost.id);
    sp.seen = true;

    const px = ghost.pos.x + ghost.vel.x * dt;
    const py = ghost.pos.y + ghost.vel.y * dt;
    const s = this.worldToScreen({ x: px, y: py });
    sp.container.position.set(s.x, s.y);

    const own = ghost.own;
    const angle = Math.atan2(ghost.vel.y, ghost.vel.x);

    // Uncertainty cone: where the object could be NOW given how stale the sighting
    // is. This is ON-DEMAND inspection detail only — shown when you SELECT a contact
    // or it is your current intercept TARGET (its staleness is exactly what tells you
    // how risky the intercept is). It is NEVER drawn ambiently: own ships no longer
    // carry an always-on uncertainty circle (that was clutter that didn't help), so
    // the map stays clean around your fleets and the cone is never confused with the
    // teal sensor bubbles. (The threat ring and selection ring below are unaffected.)
    sp.cone.clear();
    const inspecting = state.selectedShipId === ghost.id || Object.values(state.raids).includes(ghost.id);
    if (inspecting && ghost.uncertainty > 0) {
      const rPx = ghost.uncertainty * this.scale;
      sp.cone.circle(0, 0, rPx).fill({ color: COL_CONE, alpha: 0.05 }).stroke({ width: 1, color: COL_CONE, alpha: 0.22 });
    }
    // §order-lifecycle: is this own fleet's LATEST order still unconfirmed (its
    // compliance light hasn't returned)? While so, the commanded-heading hint is
    // drawn DASHED (= commanded/claimed) and an echo-pending clock badge shows;
    // both resolve to the normal SOLID hint / no badge at echo (= observed).
    const pend = own ? state.pendingOrders.get(ghost.id) : undefined;
    const liveSim = state.simTime + (performance.now() - state.lastViewWallMs) / 1000;
    const unconfirmed = !!pend && pend.echo_at - pend.delivered_at >= 1.5 && liveSim < pend.echo_at;

    // Own ship under orders: it's executing a course YOU set, so hint where it has
    // most likely advanced — from the ghost, along the commanded heading, up to how
    // far it could have moved (its uncertainty). Reads as "proceeding on last
    // orders," not "lost ship." DASHED while the order is unconfirmed.
    if (own && ghost.uncertainty > 1) {
      const dest = state.orders[ghost.id];
      if (dest) {
        const dx = dest.x - ghost.pos.x;
        const dy = dest.y - ghost.pos.y;
        const d = Math.hypot(dx, dy);
        if (d > 1) {
          const step = Math.min(ghost.uncertainty, d);
          const pr = this.worldToScreen({ x: ghost.pos.x + (dx / d) * step, y: ghost.pos.y + (dy / d) * step });
          const ox = pr.x - s.x;
          const oy = pr.y - s.y;
          if (unconfirmed) {
            dashedLine(sp.cone, 0, 0, ox, oy, 4, 4);
            sp.cone.stroke({ width: 1, color: COL_OWN, alpha: 0.45 });
          } else {
            sp.cone.moveTo(0, 0).lineTo(ox, oy).stroke({ width: 1, color: COL_OWN, alpha: 0.3 });
          }
          sp.cone.circle(ox, oy, 2.6).stroke({ width: 1.2, color: COL_OWN, alpha: 0.6 });
        }
      }
    }

    // Echo-pending badge: a small hollow clock, own-cyan, just off the pip while
    // the order is unconfirmed — a subtle state tag, not an alarm. Gone at echo.
    if (unconfirmed) {
      const bx = 11;
      const by = -(this.shipHitRadius(ghost.kind) + 5);
      sp.cone.circle(bx, by, 3.6).stroke({ width: 1.2, color: COL_OWN, alpha: 0.85 });
      // two little hands
      sp.cone.moveTo(bx, by).lineTo(bx, by - 2.4).stroke({ width: 1, color: COL_OWN, alpha: 0.85 });
      sp.cone.moveTo(bx, by).lineTo(bx + 1.8, by).stroke({ width: 1, color: COL_OWN, alpha: 0.85 });
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
    const fade = Math.min(ghost.age / FADE_AGE_S, 1);
    const alpha = own ? Math.max(0.62, 0.97 - 0.4 * fade) : Math.max(0.4, 0.95 - 0.55 * fade);
    const tex = this.texFor(ghost.kind);
    sp.body.clear();
    if (tex) {
      sp.sprite.visible = true;
      if (sp.sprite.texture !== tex) sp.sprite.texture = tex;
      // Size vs zoom: a small indicator through normal zoom, ramping to TRUE
      // NATIVE texture size in the deepest band (see shipSizePx). scale =
      // targetPx / native, so it's ≤ 1.0 everywhere — downscale-or-native, always
      // crisp, and exactly 1:1 undistorted art at max zoom.
      const targetPx = this.shipSizePx(ghost.kind, tex.width);
      sp.sprite.scale.set(targetPx / tex.width);
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
    const pipCol = own ? COL_OWN : COL_OTHER;
    const half = this.shipHitRadius(ghost.kind); // half the ship's current on-screen size
    const pipR = Math.max(3.2, Math.min(8, half * 0.14));
    const pipY = -(half + pipR + 5); // just above the sprite's top edge, at every zoom
    const pipA = Math.max(0.85, 0.97 - 0.25 * fade); // high floor — survives staleness
    const diamond = (cy: number, rr: number): number[] => [0, cy - rr, rr, cy, 0, cy + rr, -rr, cy];
    pip.poly(diamond(pipY, pipR + 1.3)).fill({ color: 0x05070d, alpha: 0.7 * pipA }); // dark rim for contrast
    pip.poly(diamond(pipY, pipR)).fill({ color: pipCol, alpha: pipA });

    // Label: threat warning for raiders, cargo manifest for convoys (shown only
    // when known — i.e. within sensor range), staleness everywhere it matters.
    const sel = state.selectedShipId === ghost.id;
    // Honest staleness, shown finer-grained when fresh (near the command center).
    const stale = `Δ${ghost.age.toFixed(ghost.age < 10 ? 1 : 0)}s`;
    let txt = "";
    let col = COL_OTHER;
    let lalpha = 0.85;
    if (ghost.kind === "raider" && !own) {
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
        ? (ghost.cargo ? `${ghost.cargo.commodity} ×${ghost.cargo.units}  ` : "")
        : "";
      txt = `${cargo}${stale}`;
      col = COL_OWN;
      lalpha = sel ? 0.95 : 0.7;
    } else if (ghost.kind === "convoy") {
      const cargo = ghost.cargo ? `${ghost.cargo.commodity} ×${ghost.cargo.units}` : "cargo ?";
      txt = `${cargo}  ${stale}`;
      col = ghost.cargo ? COL_REPORT : COL_OTHER; // known cargo = gold (intel!)
      lalpha = 0.9;
    }
    sp.label.text = txt;
    sp.label.style.fill = col;
    sp.label.alpha = lalpha;
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
    if (badgeStr) {
      const halfB = this.shipHitRadius(ghost.kind);
      const w = Math.max(13, badgeStr.length * 6 + 7);
      const h = 12;
      const bx = halfB * 0.66;
      const by = halfB * 0.55;
      const edge = own ? COL_OWN : COL_OTHER;
      const bAlpha = Math.max(0.85, 0.97 - 0.25 * fade);
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
      // Redraw the world-anchored background (rings + hub) when the camera moved.
      if (this.viewDirty) {
        this.drawBackground();
        this.viewDirty = false;
      }
      const dt = Math.min((performance.now() - state.lastViewWallMs) / 1000, MAX_EXTRAPOLATE_S);

      this.drawSensorCoverage(state, dt);
      this.drawSystems(state);
      this.drawHubBody();
      this.drawRoutes(state);
      this.drawAnchors(state);
      this.drawCommandCenter(state);

      for (const sp of this.ghosts.values()) sp.seen = false;
      const screenById = new Map<string, { x: number; y: number }>();
      for (const ghost of state.ghosts) {
        screenById.set(ghost.id, this.drawGhost(ghost, state, dt));
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

      this.drawOrders(state, screenById);
      this.drawIntercepts(state);
      this.drawSignals(state, dt);
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

  /// Advance the crossfade/camera-push. Mutates the galaxy camera + both scene
  /// alphas; finalizes (hides the inactive scene, restores the exact camera on
  /// exit) when complete. Returns which scenes to draw this frame.
  private tickTransition(): { drawGalaxy: boolean; drawSystem: boolean } {
    const tr = this.transition;
    if (!tr) return { drawGalaxy: this.mode.type === "galaxy", drawSystem: this.mode.type === "system" };

    const raw = clamp01((performance.now() - tr.start) / TRANS_MS);
    const p = easeInOut(raw);
    this.cx = tr.camFrom.cx + (tr.camTo.cx - tr.camFrom.cx) * p;
    this.cy = tr.camFrom.cy + (tr.camTo.cy - tr.camFrom.cy) * p;
    this.scale = tr.camFrom.scale + (tr.camTo.scale - tr.camFrom.scale) * p;
    this.viewDirty = true; // the camera moved — the galaxy background must redraw

    if (tr.dir === "in") {
      this.galaxyRoot.alpha = 1 - clamp01((raw - 0.35) / 0.65);
      this.systemScene.root.alpha = clamp01((raw - 0.25) / 0.75);
    } else {
      this.galaxyRoot.alpha = clamp01((raw - 0.25) / 0.75);
      this.systemScene.root.alpha = 1 - clamp01((raw - 0.35) / 0.65);
    }

    if (raw >= 1) {
      if (tr.dir === "in") {
        this.galaxyRoot.alpha = 0;
        this.galaxyRoot.visible = false; // system view is live — stop drawing the galaxy
        this.systemScene.root.alpha = 1;
      } else {
        this.galaxyRoot.alpha = 1;
        this.galaxyRoot.visible = true;
        this.systemScene.root.visible = false;
        this.systemScene.root.alpha = 1;
        this.cx = tr.camTo.cx; this.cy = tr.camTo.cy; this.scale = tr.camTo.scale; // restore exactly
      }
      this.transition = null;
    } else {
      this.galaxyRoot.visible = true;
    }
    return { drawGalaxy: true, drawSystem: true };
  }

  /// Draw the OUTBOUND command signal (server-timed; we only place it at its
  /// interpolated `pOut`): the violet comet of an order in flight, command center
  /// → ship. This is the ONE thing the map can't show — your command crossing
  /// space, not yet arrived. The ship's REACTION needs no signal: it's seen
  /// directly on the map (in delayed light) when the ghost changes course. So
  /// there is no inbound/response leg, and raid results are a notification only.
  private drawSignals(state: ViewState, dt: number): void {
    const g = this.signalsGfx;
    g.clear();
    if (!state.commandCenter) return;
    const cc = this.worldToScreen(state.commandCenter);

    // OUTBOUND only: a violet comet, command center → ship. No return leg (the
    // ship's reaction is seen on the map), and no inbound result rings (a raid
    // outcome is seen on the map + a notification) — only what the map can't show.
    for (const sig of state.commandSignals) {
      const ghost = state.ghosts.find((x) => x.id === sig.shipId);
      if (!ghost) continue;
      const gp = this.worldToScreen({ x: ghost.pos.x + ghost.vel.x * dt, y: ghost.pos.y + ghost.vel.y * dt });

      const p = Math.max(0, Math.min(1, sig.pOut));
      const hx = cc.x + (gp.x - cc.x) * p;
      const hy = cc.y + (gp.y - cc.y) * p;
      const d = norm(gp.x - hx, gp.y - hy);
      dashedLine(g, cc.x, cc.y, hx, hy, 6, 7);
      g.stroke({ width: 1, color: COL_COMMAND, alpha: 0.16 });
      for (let k = 1; k <= 4; k++) {
        g.circle(hx - d.x * k * 6, hy - d.y * k * 6, 4.4 - k * 0.8).fill({ color: COL_COMMAND, alpha: 0.42 - k * 0.08 });
      }
      g.circle(hx, hy, 12).fill({ color: COL_COMMAND, alpha: 0.12 });
      g.circle(hx, hy, 5).fill({ color: COL_COMMAND, alpha: 0.98 });
      arrowhead(g, hx + d.x * 6, hy + d.y * 6, d.x, d.y, 9, COL_COMMAND, 0.98);
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

// A dashed circle (screen px), for the platform protection ring — distinct from
// the solid sensor-bubble strokes.
function dashedCircle(g: Graphics, cx: number, cy: number, r: number, dash: number, gap: number): void {
  if (r < 4) return;
  const step = (dash + gap) / r; // radians per dash+gap
  for (let a = 0; a < Math.PI * 2; a += step) {
    const b = Math.min(a + dash / r, Math.PI * 2);
    g.moveTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
    // Approximate the arc with a couple of segments (short dashes → fine).
    const mid = (a + b) / 2;
    g.lineTo(cx + Math.cos(mid) * r, cy + Math.sin(mid) * r);
    g.lineTo(cx + Math.cos(b) * r, cy + Math.sin(b) * r);
  }
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

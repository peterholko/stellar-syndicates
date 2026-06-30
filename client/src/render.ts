// Pixi.js renderer. Draws the player's DELAYED, FOGGED view (§6) — the heart of
// the game made visible (Pillar 2: never hide the lag). Each ship is a ghost at
// the position its arriving light shows; EVERY ghost — own or enemy — carries an
// uncertainty cone (how far it could have moved since the light left) and an age
// label, and fades with staleness. There is no FTL tether to your own fleet:
// certainty comes from PROXIMITY to the command center, so a distant own ship is
// just as fogged as a distant enemy, while one nearby is crisp. An own ship under
// orders also shows a hint of where it has most likely advanced along its course.
// The command center is your vantage — the origin of everything you can see.

import { Application, Container, Graphics, Text, TextStyle } from "pixi.js";
import type { Commodity, GalaxyInfo, GhostView, Vec2 } from "./protocol";
import type { ViewState } from "./state";

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
  body: Graphics;
  label: Text;
  ring: Graphics; // selection ring
  seen: boolean;
}

export class Renderer {
  private app = new Application();
  private bg = new Container();
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
    this.app.stage.addChild(
      this.bg,
      this.sensorGfx, // soft sensor coverage, under everything gameplay
      this.systemsLayer,
      this.anchorsLayer,
      this.routesGfx, // convoy broadcast routes, under ghosts
      this.orderLayer,
      this.interceptGfx, // soft intercept estimate, under the ghosts it guides
      this.ghostsLayer,
      this.signalsLayer,
    );
    this.signalsLayer.addChild(this.signalsGfx);
    this.drawStarfield();
    window.addEventListener("resize", () => this.recompute());
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

  setGalaxy(galaxy: GalaxyInfo): void {
    this.galaxy = galaxy;
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
    this.bg.addChildAt(stars, 0);
  }

  private drawBackground(): void {
    while (this.bg.children.length > 1) this.bg.removeChildAt(1);
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
      const dotCol = mine ? COL_OWN : rival ? COL_OTHER : COL_SYSTEM;
      g.circle(s.x, s.y, 2.4).fill({ color: dotCol, alpha: 0.95 });
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

  /// Soft, fuzzy INTERCEPT ESTIMATES for committed raids (§7/§8). Computed
  /// CRUDELY — a constant-velocity projection from the delayed ghosts, ignoring
  /// acceleration and the light-delayed steer-and-correct pursuit — so it is
  /// EXPECTED to drift from the real outcome. Rendered in the sensor-circle idiom
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

        // Crude constant-velocity intercept: ETA ≈ range / cruise (a 0.7 fudge
        // for the acceleration ramp), then project the target forward.
        const range = Math.hypot(t.pos.x - r.pos.x, t.pos.y - r.pos.y);
        const eta = range / (raiderSpeed * 0.7);
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

  /// Sensor coverage: a soft bubble around each of the player's assets (their
  /// own ships + command center), radius = the server-provided sensor range.
  /// The union shows where the player can detect raiders and read cargo — and,
  /// by what it doesn't cover, where they are blind.
  private drawSensorCoverage(state: ViewState, dt: number): void {
    const g = this.sensorGfx;
    g.clear();
    if (!state.galaxy || !state.commandCenter) return;
    const rPx = state.galaxy.sensor_range * this.scale;
    const centers: { x: number; y: number }[] = [state.commandCenter];
    for (const gh of state.ghosts) {
      if (gh.own) centers.push({ x: gh.pos.x + gh.vel.x * dt, y: gh.pos.y + gh.vel.y * dt });
    }
    for (const c of centers) {
      const s = this.worldToScreen(c);
      g.circle(s.x, s.y, rPx).fill({ color: COL_SENSOR, alpha: 0.045 }).stroke({ width: 1, color: COL_SENSOR, alpha: 0.14 });
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

  private ghostSprite(id: string): GhostSprite {
    let sp = this.ghosts.get(id);
    if (!sp) {
      const container = new Container();
      const cone = new Graphics();
      const ring = new Graphics();
      const body = new Graphics();
      const label = new Text({ text: "", style: new TextStyle({ fill: COL_OTHER, fontFamily: "ui-monospace, monospace", fontSize: 9 }) });
      label.anchor.set(0, 0.5);
      container.addChild(cone, ring, body, label);
      this.ghostsLayer.addChild(container);
      sp = { container, cone, body, label, ring, seen: true };
      this.ghosts.set(id, sp);
    }
    return sp;
  }

  private drawGhost(ghost: GhostView, state: ViewState, dt: number): { x: number; y: number } {
    const sp = this.ghostSprite(ghost.id);
    sp.seen = true;

    const px = ghost.pos.x + ghost.vel.x * dt;
    const py = ghost.pos.y + ghost.vel.y * dt;
    const s = this.worldToScreen({ x: px, y: py });
    sp.container.position.set(s.x, s.y);

    const own = ghost.own;
    const color = own ? COL_OWN : COL_OTHER;
    const angle = Math.atan2(ghost.vel.y, ghost.vel.x);

    // Uncertainty cone: where the object could be NOW given how stale the sighting
    // is. OWN ships always show it — your distant fleet is light-delayed like
    // everything else (§6), and near the command center age→0 so it shrinks to
    // nothing. For RIVALS the cone is ON-DEMAND inspection detail: shown only when
    // you SELECT that contact or it is your current intercept TARGET (its staleness
    // is exactly what tells you how risky the intercept is). Otherwise it's hidden,
    // so the reddish cone is never ambient clutter or confused with the teal sensor
    // bubbles. (The threat ring and selection ring below are unaffected.)
    sp.cone.clear();
    const inspecting = state.selectedShipId === ghost.id || Object.values(state.raids).includes(ghost.id);
    if ((own || inspecting) && ghost.uncertainty > 0) {
      const rPx = ghost.uncertainty * this.scale;
      const cone = own ? COL_OWN : COL_CONE;
      sp.cone.circle(0, 0, rPx).fill({ color: cone, alpha: own ? 0.04 : 0.05 }).stroke({ width: 1, color: cone, alpha: own ? 0.16 : 0.22 });
    }
    // Own ship under orders: it's executing a course YOU set, so hint where it has
    // most likely advanced — from the ghost, along the commanded heading, up to how
    // far it could have moved (its uncertainty). Reads as "proceeding on last
    // orders," not "lost ship."
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
          sp.cone.moveTo(0, 0).lineTo(ox, oy).stroke({ width: 1, color: COL_OWN, alpha: 0.3 });
          sp.cone.circle(ox, oy, 2.6).stroke({ width: 1.2, color: COL_OWN, alpha: 0.6 });
        }
      }
    }
    // Detected rival raider = a threat contact (it's otherwise invisible). Make
    // it unmistakable with a pulsing alert ring — this is your only warning.
    if (!own && ghost.kind === "raider") {
      const pulse = 0.5 + 0.5 * Math.sin(performance.now() / 230);
      sp.cone.circle(0, 0, 13 + pulse * 7).stroke({ width: 1.6, color: COL_THREAT, alpha: 0.35 + 0.45 * pulse });
    }

    // Selection ring.
    sp.ring.clear();
    if (state.selectedShipId === ghost.id) {
      sp.ring.circle(0, 0, 13).stroke({ width: 1.5, color: 0xffffff, alpha: 0.8 });
    }

    // Body triangle, oriented by heading, faded by staleness for enemies.
    sp.body.clear();
    const len = ghost.kind === "convoy" ? 9 : 7;
    const wid = ghost.kind === "convoy" ? 6 : 3.5;
    // Fade with staleness — own ships too, so a distant (stale) own ship visibly
    // dims while one near the command center stays crisp. A higher floor for own
    // ships means you never "lose" your fleet — it just reports from further back.
    const fade = Math.min(ghost.age / FADE_AGE_S, 1);
    const alpha = own ? Math.max(0.62, 0.97 - 0.4 * fade) : Math.max(0.4, 0.95 - 0.55 * fade);
    sp.body.poly([len, 0, -len * 0.7, -wid, -len * 0.7, wid]).fill({ color, alpha });
    if (ghost.kind === "convoy") sp.body.circle(0, 0, 1.6).fill({ color: 0x05070d, alpha: 0.8 });
    sp.body.rotation = angle;

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

    return s;
  }

  update(state: ViewState): void {
    if (!state.galaxy) return;
    if (this.galaxy !== state.galaxy) this.setGalaxy(state.galaxy);
    // Redraw the world-anchored background (rings + hub) when the user zoomed/panned.
    if (this.viewDirty) {
      this.drawBackground();
      this.viewDirty = false;
    }

    const dt = Math.min((performance.now() - state.lastViewWallMs) / 1000, MAX_EXTRAPOLATE_S);

    this.drawSensorCoverage(state, dt);
    this.drawSystems(state);
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

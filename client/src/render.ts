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
import type { GalaxyInfo, GhostView, Vec2 } from "./protocol";
import type { ViewState } from "./state";

const COL_HUB = 0x7fd4ff;
const COL_SYSTEM = 0x4a5d7a;
const COL_OWN = 0x4fc3ff;
const COL_OTHER = 0xff7a6b;
const COL_ANCHOR_OWN = 0x9be7ff;
const COL_ANCHOR_OTHER = 0xcf9b6b;
const COL_CONE = 0xff7a6b;
const COL_COMMAND = 0xc56bff; // outbound order comet (violet)
const COL_REPORT = 0xffd24a; // inbound result rings (gold)
const COL_SENSOR = 0x3fe0c8; // sensor coverage (teal)
const COL_THREAT = 0xff4d4d; // detected raider (alert red)

const MAX_EXTRAPOLATE_S = 0.4;
const FADE_AGE_S = 45; // staleness at which an enemy ghost is most faded

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
  private ghostsLayer = new Container();
  private signalsLayer = new Container();
  private signalsGfx = new Graphics();
  private signalLabels = new Map<string, Text>();
  private ghosts = new Map<string, GhostSprite>();

  private galaxy: GalaxyInfo | null = null;
  private scale = 1;
  private cx = 0;
  private cy = 0;

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

  setGalaxy(galaxy: GalaxyInfo): void {
    this.galaxy = galaxy;
    this.recompute();
  }

  private recompute(): void {
    if (!this.galaxy) return;
    this.scale = (Math.min(this.viewW, this.viewH) * 0.46) / this.galaxy.radius;
    this.cx = this.viewW / 2;
    this.cy = this.viewH / 2;
    this.drawBackground();
    this.drawSystems();
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

  private drawSystems(): void {
    this.systemsLayer.removeChildren();
    if (!this.galaxy) return;
    const labelStyle = new TextStyle({ fill: 0x55657f, fontFamily: "ui-monospace, monospace", fontSize: 8 });
    for (const sys of this.galaxy.systems) {
      const s = this.worldToScreen(sys.pos);
      const g = new Graphics();
      g.circle(s.x, s.y, 2.2).fill({ color: COL_SYSTEM, alpha: 0.9 });
      this.systemsLayer.addChild(g);
      const t = new Text({ text: sys.name, style: labelStyle });
      t.anchor.set(0, 0.5);
      t.position.set(s.x + 5, s.y);
      t.alpha = 0.55;
      this.systemsLayer.addChild(t);
    }
  }

  private drawAnchors(state: ViewState): void {
    this.anchorsLayer.removeChildren();
    if (!this.galaxy) return;
    for (const a of state.anchors) {
      const own = a.owner !== null && a.owner === state.playerId;
      const s = this.worldToScreen(a.pos);
      const g = new Graphics();
      const color = own ? COL_ANCHOR_OWN : COL_ANCHOR_OTHER;
      if (a.owner) {
        g.circle(s.x, s.y, own ? 9 : 6).fill({ color, alpha: own ? 0.22 : 0.14 });
        g.circle(s.x, s.y, 3).fill({ color, alpha: 0.9 });
      } else {
        g.circle(s.x, s.y, 4).stroke({ width: 1, color: 0x3a4660, alpha: 0.7 });
      }
      this.anchorsLayer.addChild(g);
      if (own) {
        const t = new Text({ text: "HOME", style: new TextStyle({ fill: COL_ANCHOR_OWN, fontFamily: "ui-monospace, monospace", fontSize: 10, fontWeight: "700", letterSpacing: 2 }) });
        t.anchor.set(0.5, 1);
        t.position.set(s.x, s.y - 11);
        this.anchorsLayer.addChild(t);
      }
    }
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
    // is. Drawn for OWN ships too — your distant fleet is light-delayed like
    // everything else (§6). Near the command center age→0, so the cone shrinks to
    // nothing and the ship reads as crisp/certain.
    sp.cone.clear();
    if (ghost.uncertainty > 0) {
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

    const dt = Math.min((performance.now() - state.lastViewWallMs) / 1000, MAX_EXTRAPOLATE_S);

    this.drawSensorCoverage(state, dt);
    this.drawRoutes(state);
    this.drawAnchors(state);
    this.drawCommandCenter(state);

    for (const sp of this.ghosts.values()) sp.seen = false;
    const screenById = new Map<string, { x: number; y: number }>();
    for (const ghost of state.ghosts) {
      screenById.set(ghost.id, this.drawGhost(ghost, state, dt));
    }
    for (const [id, sp] of this.ghosts) {
      if (!sp.seen) {
        this.ghostsLayer.removeChild(sp.container);
        sp.container.destroy({ children: true });
        this.ghosts.delete(id);
      }
    }

    this.drawOrders(state, screenById);
    this.drawSignals(state, dt);
  }

  private signalLabel(id: string): Text {
    let t = this.signalLabels.get(id);
    if (!t) {
      t = new Text({
        text: "",
        style: new TextStyle({ fill: COL_COMMAND, fontFamily: "ui-monospace, monospace", fontSize: 9, fontWeight: "700", letterSpacing: 1 }),
      });
      t.anchor.set(0, 0.5);
      this.signalsLayer.addChild(t);
      this.signalLabels.set(id, t);
    }
    return t;
  }

  /// Draw the traveling communication signals (server-timed; we only place them
  /// at their interpolated `progress`). Violet = an order's round trip (comet
  /// out, then the response light home); gold rings = a raid result crossing
  /// home to you.
  private drawSignals(state: ViewState, dt: number): void {
    const g = this.signalsGfx;
    g.clear();
    if (!state.commandCenter) return;
    const cc = this.worldToScreen(state.commandCenter);

    // Order round trip: comet OUT to the ship, then the response light home.
    const liveLabels = new Set<string>();
    for (const sig of state.commandSignals) {
      const ghost = state.ghosts.find((x) => x.id === sig.shipId);
      if (!ghost) continue;
      const gp = this.worldToScreen({ x: ghost.pos.x + ghost.vel.x * dt, y: ghost.pos.y + ghost.vel.y * dt });

      if (sig.phase === "out") {
        // Violet comet: command center → ghost.
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
      } else {
        // Return leg: the order has reached the ship; now the light of its
        // maneuver is travelling back. Fill the gap so the wait is explained.
        const p = Math.max(0, Math.min(1, sig.pBack));

        // "Order received" flash at the ghost (brief, at the hand-off).
        if (p < 0.22) {
          const f = p / 0.22;
          g.circle(gp.x, gp.y, 6 + f * 22).stroke({ width: 2.4 * (1 - f), color: COL_COMMAND, alpha: 0.8 * (1 - f) });
        }

        // A faint, hollow violet pulse travelling ghost → command center.
        const px = gp.x + (cc.x - gp.x) * p;
        const py = gp.y + (cc.y - gp.y) * p;
        const d = norm(cc.x - px, cc.y - py); // heading home
        dashedLine(g, px, py, cc.x, cc.y, 5, 8);
        g.stroke({ width: 1, color: COL_COMMAND, alpha: 0.16 });
        for (let k = 0; k < 2; k++) {
          const ph = (p * 3 + k * 0.5) % 1;
          g.circle(px, py, 4 + ph * 10).stroke({ width: 1.6 * (1 - ph), color: COL_COMMAND, alpha: 0.5 * (1 - ph) });
        }
        g.circle(px, py, 2).fill({ color: COL_COMMAND, alpha: 0.85 });
        arrowhead(g, px + d.x * 9, py + d.y * 9, d.x, d.y, 6, COL_COMMAND, 0.85);

        // Status label on the ship: the pause is the return-trip delay.
        const label = this.signalLabel(sig.shipId);
        label.text = `RECEIVED · response light ~${Math.ceil(sig.remainingS)}s`;
        label.position.set(gp.x + 12, gp.y + 10);
        label.visible = true;
        liveLabels.add(sig.shipId);
      }
    }
    // Hide labels whose signal ended (the response light has arrived).
    for (const [id, label] of this.signalLabels) {
      if (!liveLabels.has(id)) label.visible = false;
    }

    // Inbound result rings: resolution point → command center.
    for (const sig of state.reportSignals) {
      const from = this.worldToScreen(sig.from);
      const p = Math.max(0, Math.min(1, sig.progress));
      const px = from.x + (cc.x - from.x) * p;
      const py = from.y + (cc.y - from.y) * p;
      const d = norm(cc.x - px, cc.y - py); // heading home
      dashedLine(g, px, py, cc.x, cc.y, 6, 7);
      g.stroke({ width: 1, color: COL_REPORT, alpha: 0.3 });
      const elapsed = sig.progress * sig.durationS;
      for (let k = 0; k < 3; k++) {
        const ph = (elapsed * 1.7 + k * 0.34) % 1;
        g.circle(px, py, 5 + ph * 17).stroke({ width: 2.2 * (1 - ph), color: COL_REPORT, alpha: 0.6 * (1 - ph) });
      }
      g.circle(px, py, 7).stroke({ width: 2.4, color: COL_REPORT, alpha: 0.95 });
      g.circle(px, py, 2).fill({ color: COL_REPORT, alpha: 0.9 });
      arrowhead(g, px + d.x * 12, py + d.y * 12, d.x, d.y, 7, COL_REPORT, 0.95);
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

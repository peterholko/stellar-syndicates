// Pixi.js renderer. Draws the player's DELAYED, FOGGED view (§6) — the heart of
// the game made visible (Pillar 2: never hide the lag). Each ship is a ghost at
// the position its arriving light shows; enemy ghosts carry an uncertainty cone
// (how far they could have moved since the light left) and an age label, and
// fade with staleness. Your own ships are coherent (no cone). The command center
// is your vantage — the origin of everything you can see.

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
  private systemsLayer = new Container();
  private anchorsLayer = new Container();
  private orderLayer = new Container();
  private ghostsLayer = new Container();
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
      this.systemsLayer,
      this.anchorsLayer,
      this.orderLayer,
      this.ghostsLayer,
    );
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

    // Uncertainty cone (enemies only): where it could be now, given staleness.
    sp.cone.clear();
    if (!own && ghost.uncertainty > 0) {
      const rPx = ghost.uncertainty * this.scale;
      sp.cone.circle(0, 0, rPx).fill({ color: COL_CONE, alpha: 0.05 }).stroke({ width: 1, color: COL_CONE, alpha: 0.22 });
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
    const alpha = own ? 0.97 : Math.max(0.4, 0.95 - 0.55 * Math.min(ghost.age / FADE_AGE_S, 1));
    sp.body.poly([len, 0, -len * 0.7, -wid, -len * 0.7, wid]).fill({ color, alpha });
    if (ghost.kind === "convoy") sp.body.circle(0, 0, 1.6).fill({ color: 0x05070d, alpha: 0.8 });
    sp.body.rotation = angle;

    // Age label — disclosure of staleness. Shown for enemies and selection.
    if (!own) {
      sp.label.text = `Δ${ghost.age.toFixed(0)}s`;
      sp.label.style.fill = COL_OTHER;
      sp.label.alpha = 0.85;
      sp.label.position.set(11, -10);
    } else if (state.selectedShipId === ghost.id) {
      sp.label.text = `Δ${ghost.age.toFixed(1)}s`;
      sp.label.style.fill = COL_OWN;
      sp.label.alpha = 0.85;
      sp.label.position.set(11, -10);
    } else {
      sp.label.text = "";
    }

    return s;
  }

  update(state: ViewState): void {
    if (!state.galaxy) return;
    if (this.galaxy !== state.galaxy) this.setGalaxy(state.galaxy);

    this.drawAnchors(state);
    this.drawCommandCenter(state);

    const dt = Math.min((performance.now() - state.lastViewWallMs) / 1000, MAX_EXTRAPOLATE_S);

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

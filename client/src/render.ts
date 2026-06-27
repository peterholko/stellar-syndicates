// Pixi.js renderer. Draws the galaxy the server streams: the wormhole hub,
// star systems, home anchors, and ships moving under flip-and-burn. In M2 these
// are TRUE positions (every client sees the same world); M3 layers on staleness.
//
// The client holds no authoritative state — it renders whatever View arrived and
// extrapolates ship positions by their velocity between the ~10 Hz server
// updates so motion looks smooth at 60 fps.

import { Application, Container, Graphics, Text, TextStyle } from "pixi.js";
import type { GalaxyInfo, ShipView, Vec2 } from "./protocol";
import type { ViewState } from "./state";

const COL_HUB = 0x7fd4ff;
const COL_SYSTEM = 0x4a5d7a;
const COL_OWN = 0x4fc3ff;
const COL_OTHER = 0xff7a6b;
const COL_ANCHOR_OWN = 0x9be7ff;
const COL_ANCHOR_OTHER = 0xcf9b6b;

/// Max seconds to extrapolate a ship past its last update (guards against jumps
/// if the stream stalls).
const MAX_EXTRAPOLATE_S = 0.5;

interface ShipSprite {
  gfx: Graphics;
  seen: boolean;
}

export class Renderer {
  private app = new Application();
  private bg = new Container();
  private systemsLayer = new Container();
  private anchorsLayer = new Container();
  private shipsLayer = new Container();
  private ships = new Map<string, ShipSprite>();

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
    this.app.stage.addChild(this.bg, this.systemsLayer, this.anchorsLayer, this.shipsLayer);
    this.drawStarfield();
    window.addEventListener("resize", () => this.recompute());
  }

  private get viewW(): number {
    return this.app.renderer.width / this.app.renderer.resolution;
  }
  private get viewH(): number {
    return this.app.renderer.height / this.app.renderer.resolution;
  }

  private worldToScreen(p: Vec2): { x: number; y: number } {
    return { x: this.cx + p.x * this.scale, y: this.cy + p.y * this.scale };
  }

  setGalaxy(galaxy: GalaxyInfo): void {
    this.galaxy = galaxy;
    this.recompute();
  }

  /// Recompute the world→screen transform to fit the galaxy, and rebuild the
  /// static layers (hub, systems, boundary).
  private recompute(): void {
    if (!this.galaxy) return;
    const margin = 0.46;
    this.scale = (Math.min(this.viewW, this.viewH) * margin) / this.galaxy.radius;
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
    // Clear previous (keep the starfield at index 0).
    while (this.bg.children.length > 1) this.bg.removeChildAt(1);
    if (!this.galaxy) return;

    const g = new Graphics();
    const rPx = this.galaxy.radius * this.scale;
    // Galaxy boundary.
    g.circle(this.cx, this.cy, rPx).stroke({ width: 1, color: 0x1c2740, alpha: 0.9 });
    // Coherence rings around the hub (purely indicative for now).
    for (const f of [0.33, 0.66]) {
      g.circle(this.cx, this.cy, rPx * f).stroke({ width: 1, color: 0x141d30, alpha: 0.8 });
    }
    // The hub: a small bright wormhole.
    const hub = this.worldToScreen(this.galaxy.hub);
    g.circle(hub.x, hub.y, 11).fill({ color: COL_HUB, alpha: 0.18 });
    g.circle(hub.x, hub.y, 6).fill({ color: COL_HUB, alpha: 0.4 });
    g.circle(hub.x, hub.y, 2.5).fill({ color: 0xffffff, alpha: 0.9 });
    this.bg.addChild(g);

    const label = new Text({
      text: "HUB",
      style: new TextStyle({ fill: COL_HUB, fontFamily: "ui-monospace, monospace", fontSize: 10, letterSpacing: 2 }),
    });
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
      t.alpha = 0.6;
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
        // Unclaimed slot: faint hollow ring.
        g.circle(s.x, s.y, 4).stroke({ width: 1, color: 0x3a4660, alpha: 0.7 });
      }
      this.anchorsLayer.addChild(g);
      if (own) {
        const t = new Text({
          text: "HOME",
          style: new TextStyle({ fill: COL_ANCHOR_OWN, fontFamily: "ui-monospace, monospace", fontSize: 10, fontWeight: "700", letterSpacing: 2 }),
        });
        t.anchor.set(0.5, 1);
        t.position.set(s.x, s.y - 11);
        this.anchorsLayer.addChild(t);
      }
    }
  }

  private shipSprite(id: string): ShipSprite {
    let sp = this.ships.get(id);
    if (!sp) {
      sp = { gfx: new Graphics(), seen: true };
      this.shipsLayer.addChild(sp.gfx);
      this.ships.set(id, sp);
    }
    return sp;
  }

  private drawShip(ship: ShipView, state: ViewState, dt: number): void {
    const sp = this.shipSprite(ship.id);
    sp.seen = true;
    const g = sp.gfx;
    g.clear();

    // Extrapolate position by velocity since the last server update.
    const px = ship.pos.x + ship.vel.x * dt;
    const py = ship.pos.y + ship.vel.y * dt;
    const s = this.worldToScreen({ x: px, y: py });

    const own = ship.owner === state.playerId;
    const color = own ? COL_OWN : COL_OTHER;
    const angle = Math.atan2(ship.vel.y, ship.vel.x);

    // Triangle pointing along velocity. Convoys larger/blunter, raiders sharper.
    const len = ship.kind === "convoy" ? 9 : 7;
    const wid = ship.kind === "convoy" ? 6 : 3.5;
    g.poly([len, 0, -len * 0.7, -wid, -len * 0.7, wid]).fill({ color, alpha: 0.95 });
    if (ship.kind === "convoy") {
      g.circle(0, 0, 1.6).fill({ color: 0x05070d, alpha: 0.8 });
    }
    g.position.set(s.x, s.y);
    g.rotation = angle;
  }

  update(state: ViewState): void {
    if (!state.galaxy) return;
    if (this.galaxy !== state.galaxy) this.setGalaxy(state.galaxy);

    this.drawAnchors(state);

    const dt = Math.min((performance.now() - state.lastViewWallMs) / 1000, MAX_EXTRAPOLATE_S);

    for (const sp of this.ships.values()) sp.seen = false;
    for (const ship of state.ships) this.drawShip(ship, state, dt);
    // Remove sprites for ships no longer present.
    for (const [id, sp] of this.ships) {
      if (!sp.seen) {
        this.shipsLayer.removeChild(sp.gfx);
        sp.gfx.destroy();
        this.ships.delete(id);
      }
    }
  }
}

import { Container, Graphics, Text } from "pixi.js";
import type { SignalsView, Vec2 } from "./protocol";
import { CONTACT_LABEL_ZOOM, contactBadgePx, hitMarker, type MarkerHit } from "./core/derive/map-markers";
import { SIGNAL_INFO } from "./core/derive/signals";

const ROUTINE = 0x8fd3ff;
const HAZARD = 0xffb454;
const FILL = 0x0b1622;

/** §signals: open signals near home, private to their corporation. A small
 * diamond, not another sensor circle: amber when Hazardous. Named up close. */
export class SignalMarkerLayer {
  readonly root = new Container();
  private gfx = new Graphics();
  private labels = new Map<string, Text>();
  private hits: MarkerHit[] = [];
  constructor() { this.root.addChild(this.gfx); }

  draw(view: SignalsView | null, project: (pos: Vec2) => Vec2, zoom = 1): void {
    const open = view?.signals.filter((s) => !s.report) ?? [];
    const ids = new Set(open.map((s) => s.id));
    for (const [id, text] of this.labels) if (!ids.has(id)) { text.destroy(); this.labels.delete(id); }
    const g = this.gfx.clear();
    this.hits = [];
    const r = contactBadgePx(zoom) * .38;
    for (const signal of open) {
      const p = project(signal.pos);
      const color = signal.hazardous ? HAZARD : ROUTINE;
      g.poly([p.x, p.y - r, p.x + r, p.y, p.x, p.y + r, p.x - r, p.y], true)
        .fill({ color: FILL, alpha: .85 }).stroke({ color, width: 1.5, alpha: .95 });
      g.circle(p.x, p.y, Math.max(1.5, r * .22)).fill({ color });
      let text = this.labels.get(signal.id);
      if (!text) {
        text = new Text({ text: "", style: { fontFamily: "sans-serif", fontSize: 11, fill: 0xb0c6d3 } });
        text.anchor.set(0.5, 0);
        this.root.addChild(text);
        this.labels.set(signal.id, text);
      }
      text.text = SIGNAL_INFO[signal.kind].name;
      text.position.set(p.x, p.y + r + 3);
      text.visible = zoom >= CONTACT_LABEL_ZOOM;
      this.hits.push({ id: signal.id, x: p.x - r, y: p.y - r, w: 2 * r, h: 2 * r });
    }
  }

  /** The signal under a screen point, if any. */
  pick(sx: number, sy: number): string | null { return hitMarker(this.hits, sx, sy, 8); }
}

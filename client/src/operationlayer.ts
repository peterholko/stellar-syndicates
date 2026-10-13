import { Container, Graphics, Sprite, Text, Texture } from "pixi.js";
import type { OperationView, SystemInfo, Vec2 } from "./protocol";
import { CONTRACT_LABEL_ZOOM, contractOnMap, contractStar, contractStyle, contractTagPx, hitMarker,
  type MarkerHit } from "./core/derive/map-markers";
import { loadMapGlyphs } from "./map-glyph-textures";
import type { MapGlyph } from "./map-glyphs.generated";

const TAG_FILL = 0x0b1220;

/** Operation contracts as tags in their issuer's colour: hollow when offered,
 * filled once joined, with a progress bar while running. A contract at a star
 * stands beside it rather than under it, and contracts at one place stack. */
export class OperationLayer {
  readonly root = new Container();
  private gfx = new Graphics();
  private glyphs = new Map<MapGlyph, Texture>();
  private tags = new Map<string, { sprite: Sprite; text: Text }>();
  private hits: MarkerHit[] = [];
  constructor() { this.root.addChild(this.gfx); }
  async load(): Promise<void> { this.glyphs = await loadMapGlyphs(); }

  draw(operations: readonly OperationView[], project: (pos: Vec2) => Vec2, zoom: number,
    systems: readonly SystemInfo[] = []): void {
    const shown = operations.filter(contractOnMap);
    const ids = new Set(shown.map(op => op.id));
    for (const [id, tag] of this.tags) if (!ids.has(id)) {
      tag.sprite.destroy(); tag.text.destroy(); this.tags.delete(id);
    }
    const g = this.gfx.clear();
    this.hits = [];
    const h = contractTagPx(zoom);
    const stacks = new Map<string, number>();
    for (const op of shown) {
      let tag = this.tags.get(op.id);
      if (!tag) {
        const sprite = new Sprite(Texture.EMPTY);
        sprite.anchor.set(0.5);
        const text = new Text({ text: "", style: { fontFamily: "ui-monospace, monospace", fontSize: 10, letterSpacing: .4, fill: 0xffffff } });
        text.anchor.set(0, 0.5);
        this.root.addChild(sprite, text);
        tag = { sprite, text }; this.tags.set(op.id, tag);
      }
      const style = contractStyle(op);
      const star = contractStar(op, systems);
      const anchor = project(star?.pos ?? op.target_pos);
      const key = star ? `star:${star.id}` : `${Math.round(anchor.x)}:${Math.round(anchor.y)}`;
      const slot = stacks.get(key) ?? 0;
      stacks.set(key, slot + 1);
      // A free-space contract's glyph marks its point; one at a star stands
      // up and to the right of the star's glyph. Later ones stack upward.
      const cx = star ? anchor.x + 14 + h / 2 : anchor.x;
      const cy = (star ? anchor.y - 14 - h / 2 : anchor.y) - slot * (h + 5);
      const labelled = op.joined || zoom >= CONTRACT_LABEL_ZOOM;
      tag.text.text = labelled
        ? style.progress !== null ? `${style.label} ${Math.round(style.progress * 100)}%` : style.label
        : "";
      tag.text.visible = labelled;
      const ink = style.filled ? TAG_FILL : style.color;
      tag.text.style.fill = ink;
      const w = h + (labelled ? (tag.text.width ?? 0) + 6 : 0);
      const x = cx - h / 2, y = cy - h / 2;
      if (style.filled) g.roundRect(x, y, w, h, 4).fill({ color: style.color, alpha: .92 });
      else g.roundRect(x, y, w, h, 4).fill({ color: TAG_FILL, alpha: .85 }).stroke({ color: style.color, width: 1.5, alpha: .95 });
      if (style.progress !== null) {
        g.rect(x, y + h + 2, w, 2.5).fill({ color: TAG_FILL, alpha: .85 });
        g.rect(x, y + h + 2, w * style.progress, 2.5).fill({ color: style.color });
      }
      const texture = this.glyphs.get(style.glyph);
      tag.sprite.visible = !!texture;
      if (texture) tag.sprite.texture = texture;
      tag.sprite.position.set(cx, cy);
      tag.sprite.width = tag.sprite.height = h - 6;
      tag.sprite.tint = ink;
      if (!texture) g.circle(cx, cy, Math.max(2, h * .14)).fill({ color: ink });
      tag.text.position.set(x + h, cy);
      this.hits.push({ id: op.id, x, y, w, h });
    }
  }

  /** The contract under a screen point, if any. */
  pick(sx: number, sy: number): string | null { return hitMarker(this.hits, sx, sy); }
}

import { Container, Graphics, Sprite, Text, Texture } from "pixi.js";
import type { ExplorationSiteView, ExplorationJournalEntry, SystemInfo, Vec2 } from "./protocol";
import { siteTitle } from "./core/derive/exploration";
import { CONTACT_LABEL_ZOOM, contactBadgePx, contactStyle, hitMarker, type MarkerHit } from "./core/derive/map-markers";
import { loadMapGlyphs } from "./map-glyph-textures";
import type { MapGlyph } from "./map-glyphs.generated";

const BADGE_FILL = 0x0b1622;
const SELECTED = 0xe7f3ff;
const PIN = 0xe7d7aa;

/** Exploration contacts as sensor circles. Only arrived contacts enter this
 * layer, and an unknown one never shows its hidden kind: until a scout
 * identifies it, it is a signal in a dashed ring. Identified sites show their
 * kind's glyph and a pip for their state; spent ones grey out. */
export class ExplorationLayer {
  readonly root = new Container();
  private gfx = new Graphics();
  private glyphs = new Map<MapGlyph, Texture>();
  private markers = new Map<string, { sprite: Sprite; text: Text }>();
  private hits: MarkerHit[] = [];
  constructor() { this.root.addChild(this.gfx); }
  async load(): Promise<void> { this.glyphs = await loadMapGlyphs(); }

  draw(sites: ExplorationSiteView[], selected: string | null, project: (pos: Vec2) => Vec2,
    journal: ExplorationJournalEntry[] = [], systems: SystemInfo[] = [], zoom = 1): void {
    const ids = new Set(sites.map(s => s.id));
    for (const [id, marker] of this.markers) if (!ids.has(id)) {
      marker.sprite.destroy(); marker.text.destroy(); this.markers.delete(id);
    }
    const g = this.gfx.clear();
    this.hits = [];
    const pins = new Set(journal.filter(e => e.pinned).map(e => e.id));
    const pin = (x: number, y: number) => g.poly([x, y - 13, x + 8, y - 13, x + 8, y, x + 4, y - 4, x, y], true)
      .fill({ color: PIN, alpha: .95 });
    for (const system of systems) if (pins.has(system.id)) {
      const p = project(system.pos);
      pin(p.x + 18, p.y - 14);
    }
    const size = contactBadgePx(zoom), r = size / 2;
    for (const site of sites) {
      let marker = this.markers.get(site.id);
      if (!marker) {
        const sprite = new Sprite(Texture.EMPTY);
        sprite.anchor.set(0.5);
        const text = new Text({ text: "", style: { fontFamily: "sans-serif", fontSize: 11, fill: 0xb0c6d3 } });
        text.anchor.set(0.5, 0);
        this.root.addChild(sprite, text);
        marker = { sprite, text }; this.markers.set(site.id, marker);
      }
      const p = project(site.pos);
      const style = contactStyle(site);
      const chosen = selected === site.id;
      if (style.unknown) {
        g.circle(p.x, p.y, r).fill({ color: BADGE_FILL, alpha: .55 });
        const dashes = 10;
        for (let i = 0; i < dashes; i++) {
          const from = i / dashes * Math.PI * 2, to = from + Math.PI * 2 / dashes * .6;
          g.moveTo(p.x + Math.cos(from) * r, p.y + Math.sin(from) * r).arc(p.x, p.y, r, from, to);
        }
        g.stroke({ color: style.ring, width: 1.5, alpha: .9 });
      } else {
        g.circle(p.x, p.y, r).fill({ color: BADGE_FILL, alpha: .88 * style.alpha })
          .stroke({ color: style.ring, width: 1.5, alpha: .95 * style.alpha });
      }
      const texture = this.glyphs.get(style.glyph);
      marker.sprite.visible = !!texture;
      if (texture) marker.sprite.texture = texture;
      marker.sprite.position.set(p.x, p.y);
      marker.sprite.width = marker.sprite.height = size * .62;
      marker.sprite.tint = style.glyphTint;
      marker.sprite.alpha = style.alpha;
      if (!texture) g.circle(p.x, p.y, Math.max(2, size * .1)).fill({ color: style.glyphTint, alpha: style.alpha });
      if (style.pip !== null) {
        g.circle(p.x + r * .72, p.y - r * .72, Math.max(3, size * .14)).fill({ color: style.pip })
          .stroke({ color: 0x070b14, width: 1.5 });
      }
      if (chosen) g.circle(p.x, p.y, r + 4).stroke({ color: SELECTED, width: 1.5, alpha: .95 });
      if (pins.has(site.id)) pin(p.x + r + 2, p.y - r);
      marker.text.text = siteTitle(site);
      marker.text.position.set(p.x, p.y + r + 3);
      marker.text.visible = chosen || pins.has(site.id) || (!style.unknown && zoom >= CONTACT_LABEL_ZOOM);
      this.hits.push({ id: site.id, x: p.x - r, y: p.y - r, w: size, h: size });
    }
  }

  /** The contact under a screen point, if any. */
  pick(sx: number, sy: number): string | null { return hitMarker(this.hits, sx, sy, 6); }
}

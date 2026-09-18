import { Assets, Container, Graphics, Sprite, Text, Texture } from "pixi.js";
import type { ExplorationKind, ExplorationSiteView, ExplorationJournalEntry, SystemInfo, Vec2 } from "./protocol";
import { EXPLORATION_MARKER_PX, siteStatus, siteTitle } from "./core/derive/exploration";

/** Only arrived contacts enter this layer. An unknown site never selects a
 * texture by its hidden kind; texture loading itself is a static public bundle. */
export class ExplorationLayer {
  readonly root = new Container();
  private gfx = new Graphics();
  private textures = new Map<ExplorationKind, Texture>();
  private markers = new Map<string, { sprite: Sprite; text: Text }>();
  constructor() { this.root.addChild(this.gfx); }
  async load(): Promise<void> {
    await Promise.all((["derelict", "station", "asteroids", "anomaly", "precursor"] as ExplorationKind[]).map(async kind => {
      try { this.textures.set(kind, await Assets.load<Texture>(`/art/exploration/${kind}.png`)); }
      catch { /* The unidentified-style fallback remains selectable. */ }
    }));
  }
  draw(sites: ExplorationSiteView[], selected: string | null, project: (pos: Vec2) => Vec2,
    journal: ExplorationJournalEntry[] = [], systems: SystemInfo[] = []): void {
    const ids = new Set(sites.map(s => s.id));
    for (const [id, marker] of this.markers) if (!ids.has(id)) {
      marker.sprite.destroy(); marker.text.destroy(); this.markers.delete(id);
    }
    const g = this.gfx.clear();
    const pins = new Set(journal.filter(e => e.pinned).map(e => e.id));
    const pin = (p: Vec2) => g.poly([p.x + 18, p.y - 27, p.x + 26, p.y - 27,
      p.x + 26, p.y - 14, p.x + 22, p.y - 18, p.x + 18, p.y - 14], true).fill({ color: 0xe7d7aa, alpha: .95 });
    for (const system of systems) if (pins.has(system.id)) pin(project(system.pos));
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
      if (pins.has(site.id)) pin(p);
      const texture = site.details ? this.textures.get(site.details.kind) : undefined;
      marker.sprite.visible = !!texture;
      if (texture) marker.sprite.texture = texture;
      marker.sprite.position.set(p.x, p.y);
      marker.sprite.width = marker.sprite.height = EXPLORATION_MARKER_PX;
      marker.sprite.alpha = siteStatus(site) === "Depleted" ? 0.38 : 0.95;
      const chosen = selected === site.id;
      if (!texture) {
        g.poly([p.x, p.y - 10, p.x + 10, p.y, p.x, p.y + 10, p.x - 10, p.y], true)
          .stroke({ color: 0xd2bd85, alpha: 0.8, width: 1.5 });
        g.circle(p.x, p.y, 2).fill({ color: 0xe7d7aa });
      }
      if (chosen) g.roundRect(p.x - 26, p.y - 26, 52, 52, 5).stroke({ color: 0xe7d7aa, width: 1.5, alpha: 0.95 });
      marker.text.text = siteTitle(site);
      marker.text.position.set(p.x, p.y + 27);
      marker.text.visible = chosen || pins.has(site.id);
    }
  }
}

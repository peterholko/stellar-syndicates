import { Assets, type Texture } from "pixi.js";
import { starArtwork, starArtLevel, type StarArtFamily, type StarArtwork } from "./starart";

export interface StarTexture {
  readonly texture: Texture;
  readonly art: StarArtwork;
  readonly size: number;
}

/** Load on demand, deduplicate requests, and retain the highest available level.
 * Zoom-out uses trilinear GPU mipmaps, not network-driven texture toggling. The
 * renderer prewarms the known zoom ceiling so ordinary wheel motion never waits
 * for a bigger image. A resize can upgrade; a late low-res response cannot undo it.
 * Asset-cache textures are shared: sprite replacement must never destroy them. */
export class StarTextureCache {
  private ready = new Map<string, StarTexture>();
  private pending = new Map<string, Promise<void>>();
  private failed = new Set<string>();
  private settled = Promise.resolve();

  constructor(private readonly changed: () => void = () => {},
    private readonly load: (url: string) => Promise<Texture> = url => Assets.load<Texture>(url)) {}

  get(family: StarArtFamily, slug: string, visibleCssPx: number, resolution: number): StarTexture | null {
    void this.ensure(family, slug, visibleCssPx, resolution);
    return this.ready.get(`${family}/${slug}`) ?? null;
  }

  ensure(family: StarArtFamily, slug: string, visibleCssPx: number, resolution: number): Promise<void> {
    const key = `${family}/${slug}`;
    const art = starArtwork(family, slug);
    const level = starArtLevel(art, visibleCssPx, resolution);
    if ((this.ready.get(key)?.size ?? 0) >= level.size || this.failed.has(level.url)) return this.settled;
    const pending = this.pending.get(level.url);
    if (pending) return pending;
    const request = this.load(level.url).then(texture => {
      // Match the full-canvas derivatives. A stale thumbnail at a larger URL
      // must not be silently accepted as high-resolution artwork.
      if (texture.width !== level.size || texture.height !== level.size) throw new Error("Wrong star texture dimensions");
      texture.source.autoGenerateMipmaps = true;
      // Pixi's scaleMode sets min/mag/mipmap filters together: smooth sampling
      // within a level and between adjacent mip levels, including Retina.
      texture.source.scaleMode = "linear";
      if ((this.ready.get(key)?.size ?? 0) < level.size) {
        this.ready.set(key, { texture, art, size: level.size });
        this.changed();
      }
    }).catch(() => {
      this.failed.add(level.url); // one failure, not a retry storm every frame
    }).finally(() => {
      this.pending.delete(level.url);
    });
    this.pending.set(level.url, request);
    return request;
  }
}

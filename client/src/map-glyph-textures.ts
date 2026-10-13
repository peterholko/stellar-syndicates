import { Assets, type Texture } from "pixi.js";
import { MAP_GLYPHS, type MapGlyph } from "./map-glyphs.generated";

let loading: Promise<Map<MapGlyph, Texture>> | null = null;

/** The map's glyph set, loaded once and shared by its layers. Glyphs are drawn
 * far below their 128 px, so they mipmap; a missing one leaves its layer's
 * vector fallback in place. */
export function loadMapGlyphs(): Promise<Map<MapGlyph, Texture>> {
  loading ??= (async () => {
    const textures = new Map<MapGlyph, Texture>();
    await Promise.all((Object.keys(MAP_GLYPHS) as MapGlyph[]).map(async name => {
      try {
        const texture = await Assets.load<Texture>(MAP_GLYPHS[name]);
        texture.source.autoGenerateMipmaps = true;
        texture.source.scaleMode = "linear";
        textures.set(name, texture);
      } catch { /* the layer draws its vector fallback */ }
    }));
    return textures;
  })();
  return loading;
}

// The Market Hub's live 3D station (the Authority Nexus). Pure rules shared by
// the map bridge, the dynamically loaded runtime, the art baker and the tests.
import { MAP_MODEL_TILT, mapModelBlend } from "./map-ship-model-policy";

/** The live tile spans this many station diameters: room for the stabilizer
 * towers when the station leans, and for the traffic arriving from beyond its
 * rim (with its wakes, up to 1.23 radii in projection mid-lean). */
export const HUB_MODEL_FIELD = 1.25;
/** The baked top-down sprite's canvas spans this many station diameters. Both
 * views share the unit, so the live tile is sized from the drawn sprite. */
export const HUB_SPRITE_FIELD = 1.02;
/** Power-of-two tiles (mipmapped). The station is drawn from 44 to 720 CSS
 * px; the bridge picks the smallest tile that covers it on this display. */
export const HUB_MODEL_TILES = [512, 1024, 2048] as const;
/** Display-sized texture copies (the source atlas is 1536 px). */
export const HUB_MODEL_TEXTURE = 1024;
export const HUB_MODEL_FADE_MS = 240;
export const HUB_MODEL_IDLE_MS = 15_000;
/** The phase the baked sprite shows, so the crossfade starts from a match. The
 * vortex flows from there, like the System View's pulsars, whatever the
 * system's reduced-motion setting. */
export const HUB_VORTEX_STILL_TIME = 0;
/** The stabilizer towers stand on the diagonals, as in the approved concept.
 * In the panel hero's low view a tower on an axis would rise in front of the
 * vortex; the map's lean clears the aperture either way. */
export const HUB_YAW = Math.PI / 4;
/** The baked panel hero looks down from the screen's bottom edge, this far
 * above the disc. */
export const HUB_HERO_ELEVATION = 35 * Math.PI / 180;

/** The same ramp as live hulls: the flat sprite below 12×, all 3D by 22×. */
export function hubModelBlend(zoom: number): number {
  return mapModelBlend(zoom);
}

/** The station's top-down rim (half its visible span) and the depth of the
 * rim's underside, in station radii, measured from the model. */
export const HUB_RIM = .9747;
export const HUB_RIM_KEEL = .0373;

/** The leaning station's lower edge below its centre, in station radii. */
export function hubLeanBottom(tilt: number): number {
  return HUB_RIM * Math.cos(tilt) + HUB_RIM_KEEL * Math.sin(Math.abs(tilt));
}

/** The live station leans like the map's hulls, seen from the screen's bottom
 * edge (negative pitch), growing with the blend from the flat sprite's pose. */
export function hubModelTilt(blend: number): number {
  return blend * MAP_MODEL_TILT;
}

/** The live tile's on-screen size, from the flat sprite's drawn canvas size:
 * at tilt 0 the live station then covers the sprite exactly. */
export function hubModelTilePx(spriteCanvasPx: number): number {
  return spriteCanvasPx * HUB_MODEL_FIELD / HUB_SPRITE_FIELD;
}

/** The asset's decorative traffic flies into the vortex once the station is
 * drawn this wide (px, its full diameter) and stops below the lower bound. */
export const HUB_TRAFFIC_ON_PX = 300;
export const HUB_TRAFFIC_OFF_PX = 260;
/** Traffic hulls at twice the asset's scale study (0.9–1.8% of the station's
 * width), so they read on the map: still small craft beside the Nexus, and
 * still clear of each other on the asset's lanes. */
export const HUB_TRAFFIC_SCALE = 2;
/** Traffic hulls are a few pixels long: display-sized textures suffice. */
export const HUB_TRAFFIC_TEXTURE = 256;

export function hubTrafficWanted(stationPx: number, current: boolean): boolean {
  return stationPx >= (current ? HUB_TRAFFIC_OFF_PX : HUB_TRAFFIC_ON_PX);
}

/** The smallest tile covering the live tile's device pixels (pixel ratio up
 * to 2). A larger tile is kept until the need falls well below it, so a zoom
 * across a boundary does not rebuild the tile at every step. */
export function hubModelTileSize(tilePx: number, pixelRatio: number, current = 0): number {
  const needed = tilePx * Math.min(Math.max(pixelRatio || 1, 1), 2);
  const fit = HUB_MODEL_TILES.find(size => size >= needed) ?? HUB_MODEL_TILES[HUB_MODEL_TILES.length - 1];
  return current > fit && needed > current * .4 ? current : fit;
}

/** The vortex redraws at up to this rate; the largest tile is a 16 MB upload. */
export function hubModelFps(tileSize: number): number {
  return tileSize >= 2048 ? 20 : 30;
}

/** Whether any of the live tile, centred on (x, y), reaches the canvas. */
export function hubModelOnScreen(x: number, y: number, tilePx: number, width: number, height: number): boolean {
  const half = tilePx / 2;
  return Number.isFinite(x + y + tilePx) && tilePx > 0
    && x + half >= 0 && y + half >= 0 && x - half <= width && y - half <= height;
}

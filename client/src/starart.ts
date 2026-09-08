import { STAR_ART } from "./star-art.generated";

export type StarArtFamily = "galaxy" | "system";
export interface StarArtGeometry {
  readonly anchor: readonly [number, number];
  readonly visualRatio: number;
}
export interface StarArtLevel {
  readonly size: number;
  readonly url: string;
}
export interface StarArtwork extends StarArtGeometry {
  readonly levels: readonly StarArtLevel[];
}

/** Separate optical map sprites and resolved system stars, with geometry
 * measured once from each master. All resolution tiers use that same geometry. */
export function starArtwork(family: StarArtFamily, slug: string): StarArtwork {
  const art = (STAR_ART[family] as Record<string, StarArtwork>)[slug];
  if (!art) throw new Error(`Missing ${family} star artwork: ${slug}`);
  return art;
}

/** A visible CSS diameter is not the texture footprint: account for transparent
 * padding AND framebuffer density. Never pick a thumbnail that needs enlarging
 * when a sufficient native tier exists. The 1254px master is the honest ceiling. */
export function starArtLevel(art: StarArtwork, visibleCssPx: number, renderResolution: number): StarArtLevel {
  const pixels = Math.max(0, visibleCssPx) / art.visualRatio * Math.max(1, renderResolution);
  return art.levels.find(level => level.size >= pixels) ?? art.levels[art.levels.length - 1];
}

/** DOM panels use the same approved detailed set; the browser selects by the
 * image's CSS canvas width and device density (not the map's visible-disk size). */
export function starArtSrcset(family: StarArtFamily, slug: string): string {
  return starArtwork(family, slug).levels.map(level => `${level.url} ${level.size}w`).join(", ");
}

export function starArtUrl(family: StarArtFamily, slug: string, canvasPixels = 256): string {
  const art = starArtwork(family, slug);
  return starArtLevel(art, canvasPixels * art.visualRatio, 1).url;
}

import { PLANET_ART } from "./planet-art.generated";

export type PlanetArtKind = keyof typeof PLANET_ART;
export interface PlanetArtGeometry {
  readonly anchor: readonly [number, number];
  readonly visualRatio: number;
}

export function planetArtwork(kind: PlanetArtKind) {
  return PLANET_ART[kind];
}

/** Resolution choice only: never use a PNG's transparent padding as planet
 * size. The renderer uses the measured master geometry for every tier. */
export function planetArtUrl(kind: PlanetArtKind, canvasPixels = 512): string {
  const art = planetArtwork(kind);
  return (art.levels.find(level => level.size >= canvasPixels) ?? art.levels[art.levels.length - 1]).url;
}

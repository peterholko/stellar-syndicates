import type { BodyView, FoundingProfile } from "./protocol";
import { hashId } from "./prng";

/** Painted scenery only. These variants do not grant deposits, slots or bonuses. */
export const HABITABLE_SURFACES = [
  "highland-plateau", "savanna-world", "volcanic-highlands", "boreal-world",
  "archipelago-world", "temperate-basin",
] as const;
export type HabitableSurface = typeof HABITABLE_SURFACES[number];
/** §belt-view: an asteroid belt is a painted field of its own type. */
export const BELT_SURFACES = ["stony-belt", "metallic-belt", "icy-belt", "shattered-belt"] as const;
export type BeltSurface = typeof BELT_SURFACES[number];
export type PlanetSurface = HabitableSurface | BeltSurface | "terrestrial" | "desert" | "ocean" | "ice" | "lava" | "barren" | "marginal";
export const isBeltSurface = (surface: string | null | undefined): surface is BeltSurface =>
  (BELT_SURFACES as readonly string[]).includes(surface ?? "");
export const FOUNDING_SURFACES: Readonly<Record<FoundingProfile, HabitableSurface>> = {
  mineral_highlands: "highland-plateau",
  verdant_lowlands: "temperate-basin",
  coastal_plains: "archipelago-world",
};
const LAND_SURFACES: readonly HabitableSurface[] = [
  "highland-plateau", "savanna-world", "volcanic-highlands", "boreal-world", "temperate-basin",
];
type SurfaceBody = Pick<BodyView, "id" | "kind" | "habitable" | "founding_camp"> & { belt?: BodyView["belt"] };

/** Stable across reports, renaming, construction, research, reloads and resizing.
 * Read only public geography and an already-arrived founding profile, never
 * survey-gated geology/deposits or live simulation state. Legacy saves need no
 * new field. Nonhabitable worlds keep their existing System View variant. */
export function planetSurface(art: string, systemId?: string, body?: SurfaceBody): PlanetSurface | null {
  // §belt-view: a belt is a painted field of its type (charted belts carry it).
  if (body?.kind === "asteroid_belt") return `${body.belt?.kind ?? "stony"}-belt`;
  // Gas giants have no ground: they use the orbital scene.
  if (body?.kind === "gas_giant" || art.includes("gas_giant")) return null;
  if (body?.habitable && systemId !== undefined) {
    if (body.founding_camp) return FOUNDING_SURFACES[body.founding_camp.profile];
    if (body.kind === "ocean") return "archipelago-world";
    return LAND_SURFACES[hashId(JSON.stringify([systemId, body.id])) % LAND_SURFACES.length];
  }
  // §marginal: a marginal ocean world borrows the ice ground until it has its own.
  if (art.includes("marginal_ocean-")) return "ice";
  return (["marginal", "terrestrial", "desert", "ocean", "ice", "lava"] as const)
    .find(kind => art.includes(`${kind}-`) || art.includes(`${kind}.`)) ?? "barren";
}

/** §planet-orbit: the planet view's paintings carry this many rows of space
 * above the original 1672 × 941 picture (1672 × 1181 in all). The original is
 * the ground frame every building site is placed in. */
export const ORBIT_SPACE_ROWS = 240;
export const GROUND_ROWS = 941;

/** Direct-source resolutions, capped at the true native size (no upscaling).
 * `orbit`: the planet view's version with space added above; the founding
 * screen keeps the 16:9 original. */
export function planetSurfaceArt(surface: PlanetSurface, orbit = false) {
  // §belt-view: belt fields are painted at the orbit size only.
  if (isBeltSurface(surface)) {
    const base = `/art/derived/planet-workbench/belt-v1/${surface}`;
    const widths = [768, 1280, 1672];
    return { src: `${base}-${widths[0]}.webp`, srcset: widths.map(width => `${base}-${width}.webp ${width}w`).join(", "), full: `${base}-1672.webp` };
  }
  const habitable = (HABITABLE_SURFACES as readonly string[]).includes(surface);
  const folder = habitable ? (orbit ? "habitable-orbit-v1" : "habitable-v1") : (orbit ? "orbit-v1" : "v1");
  const base = `/art/derived/planet-workbench/${folder}/${surface}`;
  const widths = habitable ? [768, 1280, 1672] : [768, 1536];
  return {
    src: `${base}-${widths[0]}.webp`,
    srcset: widths.map(width => `${base}-${width}.webp ${width}w`).join(", "),
    full: `${base}-${widths[widths.length - 1]}.webp`,
  };
}

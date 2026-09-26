import type { BodyView, FoundingProfile } from "./protocol";
import { hashId } from "./prng";

/** Painted scenery only. These variants do not grant deposits, slots or bonuses. */
export const HABITABLE_SURFACES = [
  "highland-plateau", "savanna-world", "volcanic-highlands", "boreal-world",
  "archipelago-world", "temperate-basin",
] as const;
export type HabitableSurface = typeof HABITABLE_SURFACES[number];
export type PlanetSurface = HabitableSurface | "terrestrial" | "desert" | "ocean" | "ice" | "lava" | "barren";
export const FOUNDING_SURFACES: Readonly<Record<FoundingProfile, HabitableSurface>> = {
  mineral_highlands: "highland-plateau",
  verdant_lowlands: "temperate-basin",
  coastal_plains: "archipelago-world",
};
const LAND_SURFACES: readonly HabitableSurface[] = [
  "highland-plateau", "savanna-world", "volcanic-highlands", "boreal-world", "temperate-basin",
];
type SurfaceBody = Pick<BodyView, "id" | "kind" | "habitable" | "founding_camp">;

/** Stable across reports, renaming, construction, research, reloads and resizing.
 * Read only public geography and an already-arrived founding profile, never
 * survey-gated geology/deposits or live simulation state. Legacy saves need no
 * new field. Nonhabitable worlds keep their existing System View variant. */
export function planetSurface(art: string, systemId?: string, body?: SurfaceBody): PlanetSurface | null {
  if (body?.kind === "gas_giant" || art.includes("gas_giant")) return null;
  if (body?.habitable && systemId !== undefined) {
    if (body.founding_camp) return FOUNDING_SURFACES[body.founding_camp.profile];
    if (body.kind === "ocean") return "archipelago-world";
    return LAND_SURFACES[hashId(JSON.stringify([systemId, body.id])) % LAND_SURFACES.length];
  }
  return (["terrestrial", "desert", "ocean", "ice", "lava"] as const)
    .find(kind => art.includes(`${kind}-`) || art.includes(`${kind}.`)) ?? "barren";
}

/** Direct-source resolutions, capped at the true native size (no upscaling). */
export function planetSurfaceArt(surface: PlanetSurface) {
  const habitable = (HABITABLE_SURFACES as readonly string[]).includes(surface);
  const base = `/art/derived/planet-workbench/${habitable ? "habitable-v1" : "v1"}/${surface}`;
  const widths = habitable ? [768, 1280, 1672] : [768, 1536];
  return {
    src: `${base}-${widths[0]}.webp`,
    srcset: widths.map(width => `${base}-${width}.webp ${width}w`).join(", "),
    full: `${base}-${widths[widths.length - 1]}.webp`,
  };
}

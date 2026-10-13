import { structureImage } from "./icons";
import type { BuildState } from "./protocol";
import type { PlanetSurface } from "./planet-surfaces";
import { SURFACE_GROUND_LAYERS, SURFACE_STRUCTURE_TIERS } from "./planet-surface-structure-art.generated";

const ORBITAL_HULLS = new Set(["large_freighter", "heavy_freighter", "bulk_freighter", "destroyer", "cruiser", "battleship"]);
const LEGACY_ORBITAL_HULLS = new Set(["destroyer", "cruiser", "battleship"]);

/** Use this world's arrived jobs, never an order draft or a completion estimate.
 * Paused/queued jobs keep their hull in the berth until an empty report arrives.
 * Old reports without a yard built large freight on the ground, not in orbit. */
export function hasOrbitalShipConstruction(builds: readonly BuildState[], bodyId: number): boolean {
  return builds.some(job => job.body_id === bodyId && ORBITAL_HULLS.has(job.key)
    && (job.yard ? job.yard === "naval_drydock" : LEGACY_ORBITAL_HULLS.has(job.key)));
}

const SURFACE_BASE = "/art/derived/planet-surface-structures/v2";
const escapeAttribute = (value: string) => value.replace(/[&<>\"]/g, char => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[char]!));
const sceneImage = (base: string, sizes: string, className: string) =>
  `<img class="${className}" src="${base}-256.webp" srcset="${base}-128.webp 128w, ${base}-256.webp 256w, ${base}-512.webp 512w" sizes="${escapeAttribute(sizes)}" alt="" draggable="false" decoding="async">`;

/** Scene-only sprites. Menus and inventory deliberately keep the established
 * icon set. Terrain is a separate, reusable layer, never baked into each tier.
 * Null surface means a gas giant/globe; space yards and attachments never draw
 * terrain. The Service Berth shares its parent Warehouse's ground. */
export function planetStructureImage(key: string, tier: number, sizes: string, occupied = false, surface: PlanetSurface | null = null): string {
  // §power: stand-in — the Fuel Refinery's scene art until the plant's own lands.
  if (key === "power_plant") key = "fuel_refinery";
  // §fuel-overhaul: stand-ins until their own art lands.
  if (key === "uranium_mill") key = "fuel_refinery";
  if (key === "antimatter_plant") key = "fuel_refinery";
  // §orbital-defense: the station hangs in the sky as its level's art.
  if (key === "orbital_defense") {
    const level = Number.isFinite(tier) ? Math.min(5, Math.max(1, Math.floor(tier))) : 1;
    return sceneImage(`/art/derived/stations/v1/level-${level}`, sizes, "icon planet-structure-art");
  }
  const shipyard = ["shipyard", "naval_drydock", "capital_slipway"].includes(key);
  const maxTier = shipyard ? 6 : Object.hasOwn(SURFACE_STRUCTURE_TIERS, key) ? SURFACE_STRUCTURE_TIERS[key] : 0;
  if (!maxTier) {
    return structureImage(key, tier, "lg", undefined, "", sizes);
  }
  const level = Number.isFinite(tier) ? Math.min(maxTier, Math.max(1, Math.floor(tier))) : 1;
  const variant = key === "naval_drydock" && occupied ? "-occupied" : "";
  const base = shipyard ? `/art/derived/planet-shipyards/v1/${key}/tier-${level}${variant}`
    : `${SURFACE_BASE}/buildings/${key}/tier-${level}`;
  const ground = surface && SURFACE_GROUND_LAYERS.includes(surface) && !isOrbitalYard(key) && key !== "service_berth"
    ? sceneImage(`${SURFACE_BASE}/grounds/${surface}`, sizes, "planet-structure-ground") : "";
  return ground + sceneImage(base, sizes, "icon planet-structure-art");
}

/** In orbit rather than on the ground: the orbital yards and the station. */
export function isOrbitalYard(key: string): boolean {
  return key === "naval_drydock" || key === "capital_slipway" || key === "orbital_defense";
}

/** Fixed image-plane anchors in the starfield above all twelve painted horizons.
 * Orbit does not consume a visible ground plot; its industrial support still
 * consumes the normal development capacity. Adding/upgrading a yard never moves
 * another structure. Gas giants also keep these stations clear of the globe.
 * `painted`: the scene is a painting with space added above its 16:9 ground
 * frame (§planet-orbit), so orbit sits in that band, at negative y. */
export function orbitalYardAddress(key: string, tier = 1, painted = false): { x: number; y: number; scale: number } | undefined {
  if (!isOrbitalYard(key)) return undefined;
  // §orbital-defense: the station grows with its level (planet-panel.css) and
  // hangs a little lower as it does, clear of the sky's top edge.
  const level = Math.min(5, Math.max(1, Math.floor(tier)));
  if (key === "orbital_defense") return { x: 50, y: (painted ? -16 : 6) + level, scale: 1 };
  return { x: key === "naval_drydock" ? 26 : 74, y: painted ? -12 : 7, scale: 1 };
}

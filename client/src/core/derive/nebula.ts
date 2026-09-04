import type { GalaxyInfo, NebulaInfo, NebulaKind, Vec2 } from "../../protocol";

export function nebulaContains(region: NebulaInfo, pos: Vec2): boolean {
  const dx = pos.x - region.center.x;
  const dy = pos.y - region.center.y;
  const sin = Math.sin(region.rotation);
  const cos = Math.cos(region.rotation);
  const x = dx * cos + dy * sin;
  const y = -dx * sin + dy * cos;
  return (x / region.radius_x) ** 2 + (y / region.radius_y) ** 2 <= 1 + 1e-12;
}

export function nebulaAt(galaxy: GalaxyInfo | null, pos: Vec2): NebulaInfo | null {
  return galaxy?.nebulas.find((region) => nebulaContains(region, pos)) ?? null;
}

/** Public, served-picture jump preview. The sim repeats the same origin test on
 * true space when the delayed order arrives. */
export function jumpRangeAt(galaxy: GalaxyInfo | null, origin: Vec2): number {
  if (!galaxy) return 0;
  let multiplier = 1;
  for (const region of galaxy.nebulas) {
    if (nebulaContains(region, origin)) multiplier = Math.max(multiplier, region.jump_range_mult);
  }
  return galaxy.jump_range * multiplier;
}

export function nebulaKindLabel(kind: NebulaKind): string {
  switch (kind) {
    case "molecular_cloud": return "Molecular cloud";
    case "ion_nebula": return "Ion nebula";
    case "dust_cloud": return "Dust cloud";
    case "supernova_remnant": return "Supernova remnant";
    case "precursor_cloud": return "Precursor cloud";
  }
}

export function nebulaEffect(region: NebulaInfo): string {
  switch (region.kind) {
    case "molecular_cloud":
      return "Volatile-rich · dark-fleet signature −35%";
    case "ion_nebula":
      return "Electronic interference · sensor reach −45% inside";
    case "dust_cloud":
      return "Resource-poor · dark-fleet signature −65%";
    case "supernova_remnant":
      return "Rare Elements · radiation signature +50%";
    case "precursor_cloud":
      return `Anomalous jump field · origin range +${Math.round((region.jump_range_mult - 1) * 100)}%`;
  }
}

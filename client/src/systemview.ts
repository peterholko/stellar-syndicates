// ============================================================================
// SYSTEM VIEW — the star's rung of the zoom ladder (galaxy → system → world).
// ============================================================================
//
// HARD BOUNDARY (read before extending this file):
//   The System View is a PRESENTATION of system-level facts, not a second scale
//   of gameplay. Ships berth AT a system, battles stand AT a system, sieges are
//   fought over a system's garrison — the sim has no intra-system positions,
//   travel or targeting. Everything spatial here is cosmetic:
//     • orbital motion is a pure function of (public seed, observed time);
//     • a berthed fleet parks beside the world hosting the facility it uses;
//     • the near zone draws the star's immediate neighbourhood (dock radius →
//       the defenses' reach) to scale, keeping each contact's true bearing.
//   Making bodies spatial targets would be a MAJOR sim/protocol decision — not
//   something to slip in here.
//
// THE RUNG'S JOB: what is happening at this star right now — who is docked,
// what is coming, what is being built, what is being fought over. Per-building
// detail belongs to the world rung (the planet stage); the system rail lists.
//
// DETERMINISM & FOG:
//   `buildVisualSystem` derives the schematic SHAPE deterministically from the
//   public system id + the wire roster, so every player sees the same geography.
//   DYNAMIC state arrives from the caller's light-gated per-player view: owner-
//   only body data (structures, population) is fogged SERVER-side (a rival's
//   bodies arrive empty), so nothing drawn here can leak. A remote system is
//   posed at its OBSERVED time — its picture visibly runs behind.
// ============================================================================

import { Assets, Container, Graphics, Sprite, Text, TextStyle, Texture } from "pixi.js";
import type { BodyView, BuildState, Commodity, ConverterStatusView, Deposit, PlayerId, SystemInfo, SystemStateView, Vec2 } from "./protocol";
import { starAnchor, starTypeFor, starVisualRatio, type StarType } from "./stars";
import type { StarArtGeometry } from "./starart";
import { planetArtwork, planetArtUrl } from "./planetart";
import { hashId, mulberry32 } from "./prng";
import {
  NEAR_INNER, ORBIT_TILT, PLATFORM_COVER_SU, formatAge, moonPeriod, nearOuterFor, nearRadius,
  orbitalAngle, planetPeriod,
} from "./systemlayout";
import {
  drawCityLights, drawOrbitalGlyph, drawRings, glowSpriteTexture, hasRings, orbitalItems, orbitalYardUrl, stationArtUrl,
  terminatorTexture, towardStarRotation, type OrbitalItem,
} from "./systembodyfx";
import { StarFx } from "./systemstarfx";
import { SystemTrafficLayer, type ShipArtResolver, type TrafficModelCandidate, type TrafficPick } from "./systemtrafficlayer";
import type { SystemPicture } from "./systemtraffic";
import { hasOrbitalShipConstruction } from "./planet-structure-art";
import { label } from "./icons";

// ---- Public presentation data model (client-side, non-authoritative) --------

export type PlanetKind =
  | "terrestrial"
  | "desert"
  | "ocean"
  | "ice"
  | "gas_giant"
  | "lava"
  | "barren"
  | "marginal"
  | "marginal_ocean";

export interface CameraRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

// The established System View star size. Galaxy-map bloom must meet this
// existing visible diameter; the planetary scene itself is not resized.
const SYSTEM_STAR_VISIBLE_UNITS = 0.17;
/** Scene scale = this × the camera rectangle's short side. */
const SCENE_FIT = 0.42;
/** Largest planet radius (normalized) — always smaller than the star's disc. */
const PLANET_RADIUS_CAP = 0.078;
/** Furthest the orrery camera may zoom toward a world before the world rung. */
export const SYSTEM_ZOOM_MAX = 2.8;

export interface VisualMoon {
  id: string;
  name: string;
  orbitRadius: number; // normalized units, around its parent planet
  radius: number;
  angle: number;
  deposits: Deposit[];
  /// §bodies: structures ON this moon (owner-only upstream; {} for rivals).
  structures: Record<string, number>;
}

export interface VisualPlanet {
  id: string;
  name: string;
  kind: PlanetKind;
  orbitRadius: number; // normalized units, around the star (0 = star, ~0.95 = edge)
  radius: number; // normalized units
  angle: number; // radians (the orbital phase at time 0)
  moons: VisualMoon[];
  deposits: Deposit[];
  habitable: boolean;
  /// §bodies: structures ON this body (owner-only upstream; {} for rivals).
  structures: Record<string, number>;
}

export type BeltKindName = "stony" | "metallic" | "icy" | "shattered";

/// §belts: a charted asteroid belt — a real body (mined by a Mining Complex),
/// drawn as a ring of rocks. Uncharted belts never reach the client.
export interface VisualBelt {
  id: string;
  name: string;
  kind: BeltKindName;
  radius: number; // normalized orbit radius, nudged into a gap between planet orbits
  width: number;
  deposits: Deposit[];
  /// §pockets: the hidden pockets YOUR corporation has found here.
  pockets: Deposit[];
}

export interface VisualSystem {
  systemId: string;
  /** Public system name (the header); scenery copies may blank it. */
  name?: string;
  starType: string; // slug (stars.ts) — matches the galaxy-map icon
  planets: VisualPlanet[];
  asteroidBelts: VisualBelt[];
}

// A body the details panel can describe — planets AND moons flow through this so
// the caller needs no knowledge of the internal shape.
export interface SystemBodyDetail {
  /// The visual body's stable id (the wire body id as a string).
  id: string;
  name: string;
  kindLabel: string;
  kindColor: number;
  isMoon: boolean;
  habitable: boolean;
  deposits: Deposit[];
  description: string;
  /// The body's art (for the details-panel thumbnail); null → color swatch only.
  icon: string | null;
}

/** Everything a click in the System View can resolve to. */
export type SystemScenePick = TrafficPick | { kind: "body"; detail: SystemBodyDetail };

// ---- Per-kind presentation (color + flavor). Descriptions are flavor only. ---

interface KindMeta {
  label: string;
  color: number; // base disc color
  hi: number; // sunlit highlight
  desc: string;
}
const KIND_META: Record<PlanetKind, KindMeta> = {
  terrestrial: { label: "Terrestrial world", color: 0x6c9a6a, hi: 0xb7d8a2, desc: "A temperate world of land and sea that colonists can settle." },
  desert: { label: "Desert world", color: 0xc39a5b, hi: 0xe8cf93, desc: "A parched, dune-wrapped world baked by its star." },
  ocean: { label: "Ocean world", color: 0x4a86c4, hi: 0x9fd0f2, desc: "A blue world of deep seas and island chains that colonists can settle." },
  ice: { label: "Ice world", color: 0x9fc4d6, hi: 0xe6f4fb, desc: "A frozen world of ice plains and volatile frosts." },
  gas_giant: { label: "Gas giant", color: 0xc98f52, hi: 0xecc78a, desc: "A banded giant — a natural well of fuel and volatiles." },
  lava: { label: "Lava world", color: 0xc2502f, hi: 0xffb14a, desc: "A molten world, crust cracked with glowing magma." },
  barren: { label: "Barren world", color: 0x8a8f9c, hi: 0xc2c8d4, desc: "An airless rock — cratered, still, and mineral-rich." },
  marginal: { label: "Marginal world · can't be settled", color: 0xa29d7c, hi: 0xd9d3b4, desc: "A cold, dry world with thin air and shrunken seas. Colonists can't settle it." },
  marginal_ocean: { label: "Marginal ocean · can't be settled", color: 0x8fb1c2, hi: 0xd7e6ee, desc: "Cold, clouded seas under thin air. Colonists can't settle it." },
};

// ---- Planet/moon/asteroid ART (§planet-art) -----------------------------------
// Regenerated planet/moon masters have broad readable forms, direct-downsampled
// resolution tiers, and measured disk geometry. A 512px source covers the
// schematic through Retina/4K; trilinear GPU mipmaps prefilter it down.
const PLANET_ART_URL = (kind: PlanetKind) => planetArtUrl(kind);
const MOON_ART_URL = planetArtUrl("moon");
export const CHUNK_ART_URL = "/art/celestial_sprites/planets/asteroid_belt_chunk.png";

/// §belts: each type's own chunk art (the Stony type keeps the original
/// sprite). 256 px for the scattered chunks, 512 px for the belt's panel.
export function beltChunkUrl(kind: BeltKindName, size: 256 | 512 = 256): string {
  return kind === "stony" ? CHUNK_ART_URL : `/art/derived/belt-chunks/v1/${kind}-${size}.png`;
}

/// §belts: each belt type's label, flavor and look. `tint` recolours the
/// Stony chunk only if a type's own art fails to load.
export const BELT_META: Record<BeltKindName, { label: string; tint: number; dust: number; desc: string }> = {
  stony: { label: "Stony belt", tint: 0xffffff, dust: 0x6a7488, desc: "A belt of pitted stone — silicates, olivine and iron." },
  metallic: { label: "Metallic belt", tint: 0xb9c6d8, dust: 0x7d8ba0, desc: "Dense metal-rich rocks: iron, copper ores and ilmenite." },
  icy: { label: "Icy belt", tint: 0xd6ecff, dust: 0x8fb6d8, desc: "Frozen rubble — a reservoir of volatiles for fuel and polymers." },
  shattered: { label: "Shattered world", tint: 0xe39a7e, dust: 0x8a5a4c, desc: "The broken crust of a lost planet, rich in metal." },
};
const beltKind = (kind: string | undefined): BeltKindName =>
  kind === "metallic" || kind === "icy" || kind === "shattered" ? kind : "stony";
const CHUNK_ART_FILL = 0.43;
/// Chunk sprites scattered along each belt ring (over the existing dust dots).
const BELT_CHUNKS = 22;

// Fallback star tints (used only when the star icon texture hasn't loaded).
// Exported: the battle theater reuses them for its system-battle backdrop.
export const STAR_TINT: Record<string, number> = {
  red_dwarf: 0xff8a5c, yellow_star: 0xffe08a, white_star: 0xdfe8ff, blue_giant: 0x9fc4ff,
  red_giant: 0xff7a5c, white_dwarf: 0xcfe0ff, neutron_star: 0xbfe0ff, binary_star: 0xffd98a,
  black_hole: 0x40507a, magnetar: 0xc9a0ff,
};

const COMMODITY_COLOR: Record<Commodity, number> = {
  cuprite_ore: 0xc88757, titanium_ore: 0x8bafcf, crystalline_ore: 0x78d9e6,
  rare_metal_ore: 0xb18ad1, conductive_metals: 0xe3a773, titanium: 0xb8d9eb,
  olivine_ore: 0x9fbf5a, bornite_ore: 0x9a7fd6, ilmenite_ore: 0x8a8f99,
  iridium_ore: 0xc7d2e0, helium3_ice: 0xa8f0ff, void_crystal: 0xd98cff,
  iridium: 0xe2e8f2, helium3: 0x9ff7ff, exotic_matter: 0xf0a6ff,
  metallic_ore: 0xb0894f, rare_elements: 0xffd76b, silicates: 0xd8c9a3, volatiles: 0x6bd0ff, biomass: 0x7fdc8a,
  alloys: 0xc99bff, electronics: 0x6be2d8, polymers: 0xe89ad1, deuterium: 0xff9d5c, provisions: 0x9fe08a,
  machinery: 0x9fb0c8, armaments: 0xff7a6b,
  composites: 0x78c6b7, hull_sections: 0xb5c9dd, precision_components: 0xe9ca75, warp_core: 0x85baff,
  uranium: 0x9be35a, antimatter: 0xff5cd6, fusion_core: 0xffe08a,
};

// ---- Deterministic RNG (public geography — same for every player) ------------
// Shared home: client/src/prng.ts (same functions, bit-identical).

/// A readable schematic: worlds are drawn several times larger than any honest
/// scale would allow, gas giants larger still, and none outgrows the star. Each
/// body still makes exactly one draw here, so no other layout feature rerolls.
function radiusForKind(kind: PlanetKind, rng: () => number): number {
  if (kind === "gas_giant") return 0.058 + rng() * 0.016;
  return 0.03 + rng() * 0.018;
}

// Public body size has a visible silhouette. The ranges still overlap, so art
// reads naturally rather than as five rigid icon sizes, but a Huge world is an
// unmistakably different discovery from a Tiny moon.
function bodySizeVisual(size: BodyView["size"]): number {
  switch (size) {
    case "tiny": return 0.70;
    case "small": return 0.85;
    case "large": return 1.18;
    case "huge": return 1.35;
    default: return 1;
  }
}

// ---- The generator -----------------------------------------------------------
//
// Deterministic from the public system id + the VIEWER'S KNOWN geology
// (§explore — `deposits` comes from the light-gated view: the exact table when
// surveyed-or-owner, EMPTY when unsurveyed, so unknown deposits cannot color
// the body labels). Produces the schematic SHAPE only.
// Ownership/dynamic state is added later by the scene — never here.
export function buildVisualSystem(sys: SystemInfo, bodies: BodyView[]): VisualSystem {
  // §bodies: the ROSTER comes from the wire (the sim owns it — kinds, names,
  // habitability, deposit placement, moon structure). Only COSMETICS are
  // derived here, per body, from a stable per-body seed: orbit spacing +
  // jitter, phases, disc radii, and the visual sub-kind for rocky worlds.
  const st = starTypeFor(sys.id);
  const planets: VisualPlanet[] = [];
  const byId = new Map<number, VisualPlanet>();
  const topLevel = bodies.filter((b) => b.parent === null && b.kind !== "asteroid_belt");
  const n = Math.max(1, topLevel.length);
  topLevel.forEach((b, i) => {
    const rng = mulberry32(hashId(`${sys.id}:${b.id}`) ^ 0xc0511c);
    const kind = visualKindFor(b, rng);
    const base = 0.22 + 0.72 * (n === 1 ? 0.5 : i / (n - 1));
    const p: VisualPlanet = {
      id: String(b.id),
      name: b.name,
      kind,
      orbitRadius: Math.min(0.95, base + (rng() - 0.5) * 0.03),
      radius: Math.min(PLANET_RADIUS_CAP, radiusForKind(kind, rng) * bodySizeVisual(b.size)),
      angle: rng() * Math.PI * 2,
      moons: [],
      deposits: b.deposits ?? [],
      habitable: b.habitable,
      structures: b.structures,
    };
    planets.push(p);
    byId.set(b.id, p);
  });
  for (const b of bodies) {
    if (b.parent === null) continue;
    const parent = byId.get(b.parent);
    if (!parent) continue;
    const rng = mulberry32(hashId(`${sys.id}:${b.id}`) ^ 0xc0511c);
    parent.moons.push({
      id: String(b.id),
      name: b.name,
      orbitRadius: parent.radius + 0.02 + rng() * 0.014 + parent.moons.length * 0.016,
      radius: Math.max(0.009, (0.011 + rng() * 0.006) * bodySizeVisual(b.size)),
      angle: rng() * Math.PI * 2,
      deposits: b.deposits ?? [],
      structures: b.structures,
    });
  }

  // §belts: charted belts come from the wire. Each sits in the gap between
  // planet orbits nearest its rolled radius (a picture, not a position), so
  // its rocks never run through a world.
  const orbits = planets.map((p) => p.orbitRadius).sort((a, b) => a - b);
  const gaps: { mid: number; room: number }[] = [];
  for (let i = 1; i < orbits.length; i++) gaps.push({ mid: (orbits[i - 1] + orbits[i]) / 2, room: orbits[i] - orbits[i - 1] });
  const used = new Set<number>();
  const belts: VisualBelt[] = bodies
    .filter((b) => b.kind === "asteroid_belt" && b.belt)
    .sort((a, b) => a.belt!.radius - b.belt!.radius)
    .map((b) => {
      let radius = b.belt!.radius;
      let width = b.belt!.width;
      const gap = gaps
        .map((g, i) => ({ g, i }))
        .filter(({ i }) => !used.has(i))
        .sort((a, c) => Math.abs(a.g.mid - radius) - Math.abs(c.g.mid - radius))[0];
      if (gap) {
        used.add(gap.i);
        radius = gap.g.mid;
        width = Math.min(width, gap.g.room * 0.35);
      }
      return { id: String(b.id), name: b.name, kind: beltKind(b.belt!.kind), radius, width, deposits: b.deposits ?? [], pockets: b.pockets ?? [] };
    });
  return { systemId: sys.id, name: sys.name, starType: st.slug, planets, asteroidBelts: belts };
}

/// The wire's collapsed kind → a stable VISUAL sub-kind (rocky worlds pick a
/// barren/desert/lava look from their per-body seed; the rest map 1:1).
function visualKindFor(b: BodyView, rng: () => number): PlanetKind {
  // Mineral Highlands is a living, habitable plateau, not an airless rock.
  if (b.founding_camp) return b.founding_camp.profile === "coastal_plains" ? "ocean" : "terrestrial";
  switch (b.kind) {
    // §marginal: only a world colonists can settle wears the lush art. Thin or
    // hostile air (such as a Terran world the one-garden rule demoted) reads Marginal.
    case "terrestrial": return b.habitable ? "terrestrial" : b.environment === "uninhabitable" ? "barren" : "marginal";
    case "ocean": return b.habitable ? "ocean" : "marginal_ocean";
    case "ice": return "ice";
    case "gas_giant": return "gas_giant";
    default: {
      const rocky: PlanetKind[] = ["barren", "desert", "lava"];
      return rocky[Math.floor(rng() * rocky.length) % rocky.length];
    }
  }
}

/** The same deterministic body artwork used by the System View. */
export function bodyArtUrl(systemId: string, body: BodyView): string {
  if (body.kind === "asteroid_belt") return beltChunkUrl(beltKind(body.belt?.kind), 512);
  if (body.parent !== null) return MOON_ART_URL;
  const rng = mulberry32(hashId(`${systemId}:${body.id}`) ^ 0xc0511c);
  return PLANET_ART_URL(visualKindFor(body, rng));
}

// ---- The scene ---------------------------------------------------------------

interface BodyHit {
  sx: number; sy: number; r: number; // screen-space center + hit radius
  detail: SystemBodyDetail;
}

/** One world in the scene: art + dressing in a depth-sorted group. */
interface BodyNode {
  id: string;
  bodyId: number;
  planet: VisualPlanet | null;
  moon: VisualMoon | null;
  parent: BodyNode | null;
  moonIndex: number;
  kind: PlanetKind | "moon";
  radius: number; // normalized
  orbitRadius: number; // around the star (planets) or parent (moons)
  baseAngle: number;
  seed: number;
  detail: SystemBodyDetail;
  group: Container;
  ringBack: Graphics | null;
  orbitBack: Container;
  disc: Graphics;
  art: Sprite | null;
  shade: Sprite | null;
  lights: Graphics;
  orbitFront: Container;
  ringFront: Graphics | null;
  orbitals: Map<string, { item: OrbitalItem; node: Sprite | Graphics; url: string | null }>;
  // Current pose (normalized local, then screen).
  lx: number;
  ly: number;
  z: number;
  hit: BodyHit;
  lightsSig: string;
}

const LABEL_STYLE = () => new TextStyle({ fill: 0x9fb0c8, fontFamily: "ui-monospace, monospace", fontSize: 10,
  dropShadow: { color: 0x02040a, alpha: 0.9, blur: 2, distance: 1 } });

// ---- Scale-aware tessellation (§orbit-ring fix) -------------------------------
// The schematic's Graphics draw in NORMALIZED units inside a worldRoot scaled to
// hundreds of pixels. Pixi picks a circle's segment count from the radius AT
// DRAW TIME, so a radius-0.5 "circle" gets a handful of segments and the scale
// magnifies it into a visible polygon. These helpers build paths with a segment
// count derived from the DISPLAYED pixel size.
const ringSegments = (pixelR: number): number => Math.max(64, Math.min(256, Math.ceil(pixelR * 0.75)));

/// A circle path (for .fill()/.stroke()) tessellated for its on-screen size.
function circlePath(g: Graphics, x: number, y: number, r: number, pixelScale: number): Graphics {
  return ellipsePath(g, x, y, r, r, pixelScale);
}

/// An axis-aligned ellipse path, same idea (tilted orbits, the gas-band fallback).
function ellipsePath(g: Graphics, x: number, y: number, rx: number, ry: number, pixelScale: number): Graphics {
  const n = ringSegments(Math.max(rx, ry) * pixelScale);
  g.moveTo(x + rx, y);
  for (let i = 1; i <= n; i++) {
    const a = (i / n) * Math.PI * 2;
    g.lineTo(x + Math.cos(a) * rx, y + Math.sin(a) * ry);
  }
  g.closePath();
  return g;
}

/// A dashed ellipse (the defenses' reach, on hover).
function dashedEllipse(g: Graphics, rx: number, ry: number, dashes: number): Graphics {
  for (let i = 0; i < dashes; i++) {
    const a0 = (i / dashes) * Math.PI * 2;
    const a1 = ((i + 0.5) / dashes) * Math.PI * 2;
    g.moveTo(Math.cos(a0) * rx, Math.sin(a0) * ry);
    const steps = 6;
    for (let k = 1; k <= steps; k++) {
      const a = a0 + (a1 - a0) * (k / steps);
      g.lineTo(Math.cos(a) * rx, Math.sin(a) * ry);
    }
  }
  return g;
}

/** Problems a world reports, condensed for its status chip (owner only). */
export interface BodyStatus {
  building: number;
  idle: number;
  warning: string | null;
}

export function bodyStatuses(report: SystemStateView | null | undefined, mainBody: number | null): Map<number, BodyStatus> {
  const out = new Map<number, BodyStatus>();
  if (!report) return out;
  const entry = (id: number) => {
    let s = out.get(id);
    if (!s) { s = { building: 0, idle: 0, warning: null }; out.set(id, s); }
    return s;
  };
  for (const job of report.builds ?? []) entry(job.body_id).building++;
  for (const converter of report.converters ?? [] as ConverterStatusView[]) {
    if (converter.status !== "running") entry(converter.body_id).idle++;
  }
  if (mainBody !== null) {
    if (report.food_state && report.food_state !== "well_supplied") entry(mainBody).warning = report.food_state.replace(/_/g, " ");
    else if (report.storage_cap > 0 && report.storage_used >= report.storage_cap) entry(mainBody).warning = "storage full";
  }
  // §power: a browning-out planet says so on its own chip.
  for (const body of report.bodies ?? []) {
    const power = body.power?.power ?? 1;
    if (power < 0.999 && !entry(body.id).warning) entry(body.id).warning = `power ${Math.round(power * 100)}%`;
  }
  return out;
}

export class SystemViewScene {
  readonly root = new Container();
  private vignette = new Graphics(); // screen-space backdrop
  /// All schematic content shares this transition-only transform. The vignette
  /// deliberately remains outside: it is a viewport backdrop, not local space.
  readonly content = new Container();
  private worldRoot = new Container(); // scaled schematic space (normalized units)
  private hazeLayer = new Container(); // nebula haze behind everything
  private coverGfx = new Graphics(); // the defenses' reach, while pointing at their world (normalized)
  private orbitsGfx = new Graphics(); // orbit ellipses + belt dust (static per layout)
  private pocketGfx = new Graphics(); // §pockets: found-pocket glints, above the belt rocks
  private beltChunks = new Container(); // asteroid-chunk sprites on the belts (static)
  /// Depth-sorted worlds and the star: bodies on the far side of their orbit
  /// pass behind the star, near-side bodies in front of it.
  private bodyStack = new Container();
  private starLayer = new Container(); // the star (sprite or procedural fallback)
  private starHolder = new Container(); // the star artwork, between its glows
  private starSprite: Sprite | null = null;
  private starGfx = new Graphics(); // procedural star fallback
  private starFx = new StarFx();
  private overlay = new Graphics(); // ownership + selection (screen-space, dynamic)
  private labels = new Container(); // screen-space labels
  private labelPool: Text[] = [];
  /// Owner-only status chips beside worlds (screen space).
  private markers = new Container();
  private chipGfx = new Graphics();
  private chipTexts: Text[] = [];
  /// Label boxes placed this pose; chips avoid them too.
  private labelRects: { x0: number; y0: number; x1: number; y1: number }[] = [];
  private traffic = new SystemTrafficLayer();
  private hud = new Container();
  private header = new Text({ text: "", style: new TextStyle({ fill: 0x9fb0c8, fontFamily: "ui-monospace, monospace", fontSize: 10, letterSpacing: 1.2,
    dropShadow: { color: 0x02040a, alpha: 0.9, blur: 2, distance: 1 } }) });

  /// §bodies: the live per-body dynamic state — the wire bodies (owner data
  /// server-fogged) + the build queue (key + body).
  private dynBodies: BodyView[] = [];
  private dynBuilds: { key: string; body_id: number }[] = [];
  private devSig = "";
  private report: SystemStateView | null = null;
  private reportMine = false;
  private statusSig = "";
  private statuses = new Map<number, BodyStatus>();

  // Planet/moon/chunk textures (lazy; the KIND_META tint circle stays as the
  // fallback until each resolves).
  private kindTex = new Map<PlanetKind, Texture>();
  private moonTex: Texture | null = null;
  private chunkTex: Texture | null = null;
  /// §belts: each belt type's own chunk art (Stony is `chunkTex`).
  private beltTex = new Map<BeltKindName, Texture>();
  private yardTex = new Map<string, Texture | null>();
  private lastStarTex: Texture | null = null;
  private lastStarGeometry: StarArtGeometry | null = null;

  private vis: VisualSystem | null = null;
  private nodes: BodyNode[] = [];
  private nodeById = new Map<string, BodyNode>();
  /// What the static Graphics were last drawn for — redraw only when the
  /// system or the displayed scale changes (never per frame).
  private gfxSystem: string | null = null;
  private gfxScale = -1;
  private coverSig = "";
  /// The pointer rests on a world whose defenses cover the near zone.
  private coverHover = false;
  private bodies: BodyHit[] = []; // screen-space hit targets (positions refreshed per pose)
  private selected: { sx: number; sy: number; r: number } | null = null;
  private selectedId: string | null = null;
  /// §body-management: the transient chip-click pulse (see `pulseBody`).
  private pulse: { id: string; until: number } | null = null;
  private viewW = 0;
  private viewH = 0;
  private cameraRect: CameraRect = { x: 0, y: 0, w: 0, h: 0 };
  /// Base scale (camera at rest) and the camera zoom/focus on top of it.
  private baseScale = 1;
  private sceneScale = 1;
  private zoom = 1;
  private zoomTarget = 1;
  private focus: Vec2 = { x: 0, y: 0 };
  private focusTarget: Vec2 = { x: 0, y: 0 };
  /// A zoomed camera follows the world it zoomed toward along its orbit.
  private follow: { id: string; lx: number; ly: number } | null = null;
  private center: Vec2 = { x: 0, y: 0 };
  private starLayout: Vec2 = { x: 0, y: 0 };
  private nearOuter = 1.4;
  /// Pose time (observed sim seconds) of the current body positions.
  private poseTime = 0;
  private lastFrameMs = 0;
  private picture: SystemPicture | null = null;
  private shipArt: ShipArtResolver | null = null;
  private battleIcon: (() => Texture | null) | null = null;
  private haze: Sprite | null = null;

  constructor(private readonly options: { sceneryOnly?: boolean } = {}) {
    this.starLayer.addChild(this.starFx.under, this.starGfx, this.starHolder, this.starFx.over);
    this.starLayer.zIndex = 0;
    this.bodyStack.sortableChildren = true;
    this.bodyStack.addChild(this.starLayer);
    this.worldRoot.addChild(this.hazeLayer, this.coverGfx, this.orbitsGfx, this.beltChunks, this.pocketGfx, this.bodyStack);
    if (!options.sceneryOnly) {
      this.markers.addChild(this.chipGfx);
      this.hud.addChild(this.header);
    }
    this.content.addChild(this.worldRoot, this.markers, this.traffic.root, this.overlay, this.labels, this.hud);
    this.root.addChild(this.vignette, this.content);
    this.root.visible = false;
    if (options.sceneryOnly) {
      // Battle backdrop: astronomy only, with no management chrome or input.
      this.root.eventMode = "none";
      this.vignette.visible = this.markers.visible = this.overlay.visible = this.labels.visible = false;
      this.traffic.root.visible = this.hud.visible = this.coverGfx.visible = false;
    }
    // Non-blocking: fallback circles render immediately; the scene rebuilds
    // once (cached thereafter) when the art lands.
    void this.loadArt();
  }

  /// Load the planet/moon/chunk textures (each independent; a missing icon
  /// simply leaves that body on its tint-circle fallback — noted, not fatal).
  private async loadArt(): Promise<void> {
    const load = async (url: string): Promise<Texture | null> => {
      try {
        const texture = await Assets.load<Texture>(url);
        texture.source.autoGenerateMipmaps = true;
        texture.source.scaleMode = "linear";
        return texture;
      } catch {
        return null;
      }
    };
    const kinds: PlanetKind[] = ["terrestrial", "desert", "ocean", "ice", "gas_giant", "lava", "barren", "marginal", "marginal_ocean"];
    const [moon, chunk, ...planets] = await Promise.all([
      load(MOON_ART_URL),
      load(CHUNK_ART_URL),
      ...kinds.map((k) => load(PLANET_ART_URL(k))),
    ]);
    this.moonTex = moon;
    this.chunkTex = chunk;
    // §belts: each type's own chunk art arrives on its own, never holding up
    // the worlds; only the belt rings are redrawn when it lands.
    for (const kind of ["metallic", "icy", "shattered"] as BeltKindName[]) {
      void load(beltChunkUrl(kind)).then((texture) => {
        if (!texture) return;
        this.beltTex.set(kind, texture);
        if (this.vis?.asteroidBelts.some((belt) => belt.kind === kind)) this.buildBelts(this.vis);
      });
    }
    kinds.forEach((k, i) => {
      const t = planets[i];
      if (t) this.kindTex.set(k, t);
    });
    // Art arrived after a system was already built → rebuild that one scene
    // (still cached; this happens at most once per session). Texture arrival is
    // not a new system selection: the picked world and transform are kept.
    if (this.vis) {
      this.buildStatic(this.vis, this.lastStarTex);
      this.layout(this.viewW, this.viewH, this.cameraRect);
    }
  }

  /// A planet kind's loaded art, or null until it arrives. The galaxy map's
  /// orbit preview draws the same worlds the System View does.
  planetTexture(kind: PlanetKind): Texture | null {
    return this.kindTex.get(kind) ?? null;
  }

  /// The renderer's ship art (formations, single hulls, pirate culture) and
  /// its battle emblem — both resolved lazily, so late-loading art appears.
  setShipArt(resolver: ShipArtResolver | null, battleIcon: (() => Texture | null) | null = null): void {
    this.shipArt = resolver;
    this.battleIcon = battleIcon;
  }

  /// Nebula haze for systems lying inside a public nebula (null clears it).
  setHaze(color: number | null): void {
    const texture = glowSpriteTexture();
    if (color === null || !texture) {
      if (this.haze) this.haze.visible = false;
      return;
    }
    this.haze ??= this.hazeLayer.addChild(new Sprite(texture));
    this.haze.visible = true;
    this.haze.anchor.set(0.5);
    this.haze.tint = color;
    this.haze.alpha = 0.16;
    this.haze.scale.set(4.2 / texture.width);
  }

  /// (Re)build the STATIC schematic for a system. No-op if already showing it.
  setSystem(vis: VisualSystem, starTex: Texture | null, starGeometry: StarArtGeometry | null = null): void {
    if (this.vis?.systemId === vis.systemId
      && (!this.options.sceneryOnly || (this.vis === vis && this.lastStarTex === starTex))) {
      this.setStarArt(starTex, starGeometry);
      return;
    }
    this.vis = vis;
    this.lastStarTex = starTex;
    this.lastStarGeometry = starGeometry;
    this.selected = null;
    this.selectedId = null;
    this.coverHover = false;
    this.zoom = this.zoomTarget = 1;
    this.focus = { x: 0, y: 0 };
    this.focusTarget = { x: 0, y: 0 };
    this.traffic.clear();
    this.picture = null;
    this.buildStatic(vis, starTex);
    this.layout(this.viewW, this.viewH, this.cameraRect);
  }

  currentId(): string | null {
    return this.vis?.systemId ?? null;
  }

  /** Late high-resolution art updates just the star, never the selected planet,
   * body hit targets, orbit geometry, or the current handoff transform. */
  setStarArt(texture: Texture | null, geometry: StarArtGeometry | null): void {
    if (this.lastStarTex === texture && this.lastStarGeometry === geometry) return;
    this.lastStarTex = texture;
    this.lastStarGeometry = geometry;
    if (!this.vis) return;
    this.syncStarSprite(texture);
    if (!texture) {
      this.gfxScale = -1;
      this.layout(this.viewW, this.viewH, this.cameraRect);
    }
  }

  private syncStarSprite(texture: Texture | null): void {
    if (!texture) {
      this.starSprite?.destroy();
      this.starSprite = null;
      return;
    }
    const st = starTypeFor(this.vis!.systemId);
    // Legacy callers (battle scenery) keep their existing icon and calibration.
    // The interactive System View supplies its own detailed-art master geometry.
    const anchor = this.lastStarGeometry?.anchor ?? starAnchor(st);
    const ratio = this.lastStarGeometry?.visualRatio ?? starVisualRatio(st);
    const sprite = this.starSprite ??= this.starHolder.addChild(new Sprite(texture));
    sprite.texture = texture;
    sprite.anchor.set(anchor[0], anchor[1]);
    sprite.scale.set(SYSTEM_STAR_VISIBLE_UNITS / (ratio * texture.width));
    this.starGfx.clear();
  }

  /// The schematic star's untransformed layout position. Transition choreography
  /// pivots here; normal System View rendering keeps `content` neutral.
  starLayoutPosition(): Vec2 {
    return { ...this.starLayout };
  }

  /// The star's visible diameter at rest (camera unzoomed): the galaxy map's
  /// deep-zoom bloom meets exactly this size at the handoff.
  starVisibleDiameterPx(): number {
    return SYSTEM_STAR_VISIBLE_UNITS * this.baseScale;
  }

  /// `starLayer` lives at normalized schematic origin; worldRoot maps that origin
  /// to `starLayout`. Inverting the parent content scale here therefore holds the
  /// sprite and procedural glow at full size about the exact star anchor.
  setStarCounterScale(contentScale: number): void {
    this.starLayer.scale.set(1 / Math.max(contentScale, 1e-6));
  }

  resetContentTransform(): void {
    this.content.pivot.set(0, 0);
    this.content.position.set(0, 0);
    this.content.scale.set(1);
    this.setStarCounterScale(1);
  }

  clearSelection(): void {
    this.selected = null;
    this.selectedId = null;
  }

  /// §bodies: feed the CURRENT wire bodies + queue. Structures/population on
  /// rival bodies arrive empty (server fog), so nothing drawn can leak. The
  /// visual bodies' structure maps refresh too (a completed build appears).
  setDynamic(bodies: BodyView[], builds: { key: string; body_id: number }[], fed: boolean): void {
    if (this.options.sceneryOnly) return;
    const sig = `${this.vis?.systemId ?? ""}|` +
      bodies.map((b) => `${b.id}:${b.population}:${Object.entries(b.structures).map(([k, t]) => `${k}${t}`).join("+")}`).join(",") +
      `|${builds.map((j) => `${j.key}@${j.body_id}`).join(",")}|${fed}`;
    if (sig === this.devSig) return; // same picture — keep the cached dressing
    this.dynBodies = bodies;
    this.dynBuilds = builds;
    this.devSig = sig;
    if (this.vis) {
      const byId = new Map(bodies.map((b) => [String(b.id), b]));
      for (const p of this.vis.planets) {
        const w = byId.get(p.id);
        if (w) { p.structures = w.structures; p.deposits = w.deposits ?? []; }
        for (const mn of p.moons) {
          const wm = byId.get(mn.id);
          if (wm) { mn.structures = wm.structures; mn.deposits = wm.deposits ?? []; }
        }
      }
    }
    this.dressBodies();
  }

  /// The full served system report (owner-only fields fogged upstream) — the
  /// status chips and the occupied-yard art read it. `mine` gates the chips.
  setReport(report: SystemStateView | null, mine: boolean): void {
    if (this.options.sceneryOnly) return;
    this.report = report;
    this.reportMine = mine;
    this.syncBeltPockets(report);
    const main = report && mine ? this.mainBodyId(report) : null;
    const statuses = mine ? bodyStatuses(report, main) : new Map<number, BodyStatus>();
    const sig = [...statuses].map(([id, s]) => `${id}:${s.building}:${s.idle}:${s.warning}`).join("|")
      + `|${(report?.builds ?? []).map((b: BuildState) => `${b.key}@${b.body_id}:${b.yard ?? ""}`).join(",")}`
      + `|${(report?.bodies ?? []).map((b) => `${b.id}:${Object.entries(b.structures).map(([k, t]) => `${k}${t}`).join("+")}`).join(",")}`;
    if (sig === this.statusSig) return;
    this.statusSig = sig;
    this.statuses = statuses;
    this.dressBodies();
  }

  /// §pockets: a find whose report lands while you look at the system glints
  /// at once: refresh each belt's pockets and redraw the band.
  private syncBeltPockets(report: SystemStateView | null): void {
    if (!this.vis || !report || report.id !== this.vis.systemId) return;
    // Reserves tick down while a pocket is worked; only a find or a pocket
    // running dry changes the picture.
    const sig = (list: Deposit[]) => list.map((p) => `${p.resource}:${p.reserves !== null && p.reserves <= 0 ? 0 : 1}`).join(",");
    let changed = false;
    for (const belt of this.vis.asteroidBelts) {
      const pockets = report.bodies.find((b) => String(b.id) === belt.id)?.pockets ?? [];
      if (sig(pockets) !== sig(belt.pockets)) changed = true;
      belt.pockets = pockets;
    }
    if (changed) this.gfxSystem = null;
  }

  private mainBodyId(report: SystemStateView): number | null {
    const populated = [...report.bodies].filter((b) => b.population > 0).sort((a, b) => b.population - a.population)[0];
    if (populated) return populated.id;
    const command = report.bodies.find((b) => (b.structures.command_center ?? 0) > 0);
    return command?.id ?? report.bodies.find((b) => b.parent === null && b.habitable)?.id ?? null;
  }

  /// The traffic picture for this frame (null clears it).
  setPicture(picture: SystemPicture | null): void {
    this.picture = picture && picture.systemId === this.vis?.systemId ? picture : null;
  }

  /// §belts: the chunk sprites along each charted belt (an INDEPENDENT seeded
  /// stream from the dust dots, so both stay deterministic and stable per
  /// system). Rebuilt alone when a belt type's art lands.
  private buildBelts(vis: VisualSystem): void {
    this.beltChunks.removeChildren().forEach((c) => c.destroy());
    for (const belt of vis.asteroidBelts) {
      const crng = mulberry32(hashId(vis.systemId + "chunks" + belt.id));
      const meta = BELT_META[belt.kind];
      // A type's own art, else the Stony chunk recoloured.
      const own = belt.kind === "stony" ? this.chunkTex : this.beltTex.get(belt.kind) ?? null;
      const tex = own ?? this.chunkTex;
      if (!tex) continue;
      for (let i = 0; i < BELT_CHUNKS; i++) {
        const ang = (i / BELT_CHUNKS) * Math.PI * 2 + crng() * 0.25;
        const rr = belt.radius + (crng() - 0.5) * belt.width;
        const size = 0.008 + crng() * 0.008;
        const sp = new Sprite(tex);
        sp.anchor.set(0.5);
        sp.position.set(Math.cos(ang) * rr, Math.sin(ang) * rr * ORBIT_TILT);
        sp.scale.set((2 * size) / (CHUNK_ART_FILL * tex.width));
        sp.rotation = crng() * Math.PI * 2;
        sp.alpha = 0.85 + crng() * 0.15;
        sp.tint = own ? 0xffffff : meta.tint;
        this.beltChunks.addChild(sp);
      }
    }
  }

  private buildStatic(vis: VisualSystem, starTex: Texture | null): void {
    this.syncStarSprite(starTex);
    this.starFx.configure(vis.starType, SYSTEM_STAR_VISIBLE_UNITS / 2);

    this.buildBelts(vis);

    // World nodes (art + dressing), rebuilt per system.
    for (const node of this.nodes) node.group.destroy({ children: true });
    this.nodes = [];
    this.nodeById.clear();
    for (const p of vis.planets) {
      const planet = this.makeNode(vis, p, null, null, 0);
      p.moons.forEach((mn, i) => this.makeNode(vis, null, mn, planet, i));
    }
    // Scenery (battle backdrops) is astronomy only: no hit targets at all.
    this.bodies = this.options.sceneryOnly ? [] : this.nodes.map((n) => n.hit);
    this.dressBodies();
    // Force the vector pass on the next layout (system changed).
    this.gfxSystem = null;
  }

  private makeNode(vis: VisualSystem, planet: VisualPlanet | null, moon: VisualMoon | null, parent: BodyNode | null, moonIndex: number): BodyNode {
    const id = planet?.id ?? moon!.id;
    const kind: PlanetKind | "moon" = planet ? planet.kind : "moon";
    const tex = planet ? this.kindTex.get(planet.kind) ?? null : this.moonTex;
    const art = planet ? planetArtwork(planet.kind) : planetArtwork("moon");
    const radius = planet?.radius ?? moon!.radius;
    const group = new Container();
    const orbitBack = new Container();
    const orbitFront = new Container();
    const disc = new Graphics();
    const lights = new Graphics();
    let sprite: Sprite | null = null;
    if (tex) {
      sprite = new Sprite(tex);
      sprite.anchor.set(art.anchor[0], art.anchor[1]);
      sprite.scale.set((2 * radius) / (art.visualRatio * tex.width));
    }
    const shadeTex = terminatorTexture();
    let shade: Sprite | null = null;
    if (shadeTex) {
      shade = new Sprite(shadeTex);
      shade.anchor.set(0.5);
      shade.scale.set((2 * radius * 1.01) / shadeTex.width);
    }
    const seed = hashId(`${vis.systemId}:${id}:dress`);
    const ringed = planet ? hasRings(planet.kind, seed) : false;
    const ringBack = ringed ? new Graphics() : null;
    const ringFront = ringed ? new Graphics() : null;
    if (ringBack) group.addChild(ringBack);
    group.addChild(orbitBack, disc);
    if (sprite) group.addChild(sprite);
    if (shade) group.addChild(shade);
    group.addChild(lights, orbitFront);
    if (ringFront) group.addChild(ringFront);
    this.bodyStack.addChild(group);
    const detail = planet ? this.planetDetail(planet) : this.moonDetail(moon!);
    const node: BodyNode = {
      id, bodyId: Number(id), planet, moon, parent, moonIndex, kind, radius,
      orbitRadius: planet?.orbitRadius ?? moon!.orbitRadius,
      baseAngle: planet?.angle ?? moon!.angle,
      seed, detail, group, ringBack, orbitBack, disc, art: sprite, shade, lights, orbitFront, ringFront,
      orbitals: new Map(), lx: 0, ly: 0, z: 0, lightsSig: "",
      hit: { sx: 0, sy: 0, r: 0, detail },
    };
    this.nodes.push(node);
    this.nodeById.set(id, node);
    return node;
  }

  /// Orbital infrastructure + night lights from the CURRENT owner data.
  /// The freshest wire roster for this star: the served report when the
  /// renderer has one for it, else the shell's last pushed bodies.
  private wireBodies(): BodyView[] {
    return this.report && this.report.id === this.vis?.systemId ? this.report.bodies : this.dynBodies;
  }

  private dressBodies(): void {
    if (this.options.sceneryOnly) return;
    const byId = new Map(this.wireBodies().map((b) => [String(b.id), b]));
    const builds = this.report?.builds ?? this.dynBuilds.map((b) => ({ ...b, complete_time: null }) as BuildState);
    for (const node of this.nodes) {
      const wire = byId.get(node.id);
      const structures = wire?.structures ?? {};
      const wanted = new Map(orbitalItems(structures, node.seed).map((item) => [item.key, item]));
      for (const [key, entry] of node.orbitals) {
        if (!wanted.has(key)) { entry.node.destroy(); node.orbitals.delete(key); }
      }
      for (const item of wanted.values()) {
        const yard = item.kind === "naval_drydock" || item.kind === "capital_slipway";
        const occupied = yard && item.kind === "naval_drydock" && hasOrbitalShipConstruction(builds, node.bodyId);
        const url = yard ? orbitalYardUrl(item.kind as "naval_drydock" | "capital_slipway", item.tier, occupied)
          : item.kind === "orbital_defense" ? stationArtUrl(item.tier) : null;
        const existing = node.orbitals.get(item.key);
        if (existing && existing.url === url && existing.item.tier === item.tier) { existing.item = item; continue; }
        existing?.node.destroy();
        let drawn: Sprite | Graphics;
        const tex = url ? this.yardTexture(url) : null;
        if (tex) {
          const sprite = new Sprite(tex);
          sprite.anchor.set(0.5);
          drawn = sprite;
        } else {
          const g = new Graphics();
          drawOrbitalGlyph(g, item.kind, item.px);
          drawn = g;
        }
        node.orbitals.set(item.key, { item, node: drawn, url });
      }
      node.lightsSig = "";
    }
    this.pose(this.poseTime);
  }

  private yardTexture(url: string): Texture | null {
    const cached = this.yardTex.get(url);
    if (cached !== undefined) return cached;
    this.yardTex.set(url, null);
    void Assets.load<Texture>(url).then((texture) => {
      texture.source.autoGenerateMipmaps = true;
      texture.source.scaleMode = "linear";
      this.yardTex.set(url, texture);
      this.dressBodies();
    }, () => { /* keep the vector stand-in */ });
    return null;
  }

  /// (Re)draw every VECTOR element of the schematic — star-fallback glow, orbit
  /// ellipses, belt dust and body fallback discs — tessellated
  /// for the CURRENT displayed size. Called only when the system or the scene
  /// scale changed — never per frame.
  private redrawGfx(vis: VisualSystem): void {
    const scale = this.sceneScale;

    // Star fallback corona (only when the star icon isn't loaded).
    this.starGfx.clear();
    if (!this.starSprite) {
      const st = starTypeFor(vis.systemId);
      const tint = STAR_TINT[st.slug] ?? 0xffe08a;
      for (const [rr, al] of [[0.13, 0.10], [0.09, 0.28], [0.06, 0.9]] as [number, number][]) {
        circlePath(this.starGfx, 0, 0, rr, scale).fill({ color: tint, alpha: al });
      }
      circlePath(this.starGfx, 0, 0, 0.03, scale).fill({ color: 0xffffff, alpha: 0.9 });
    }

    // Orbit ellipses in the tilted plane, and the belts' dust-dot grit.
    this.orbitsGfx.clear();
    this.pocketGfx.clear();
    for (const p of vis.planets) {
      ellipsePath(this.orbitsGfx, 0, 0, p.orbitRadius, p.orbitRadius * ORBIT_TILT, scale)
        .stroke({ width: 1.2 / scale, color: 0x2a3a58, alpha: 0.85 });
    }
    for (const belt of vis.asteroidBelts) {
      const dots = 90;
      const rng = mulberry32(hashId(vis.systemId + "belt" + belt.id));
      const dust = BELT_META[belt.kind].dust;
      for (let i = 0; i < dots; i++) {
        const ang = (i / dots) * Math.PI * 2 + rng() * 0.05;
        const rr = belt.radius + (rng() - 0.5) * belt.width;
        this.orbitsGfx.circle(Math.cos(ang) * rr, Math.sin(ang) * rr * ORBIT_TILT, 0.0016 + rng() * 0.0016)
          .fill({ color: dust, alpha: 0.5 });
      }
      // §pockets: each pocket you have found glints on the band in its ore's
      // colour: a four-point spark over the rocks, not another ring.
      for (const pocket of belt.pockets) {
        const ang = ((hashId(`${vis.systemId}pocket${belt.id}${pocket.resource}`) % 3600) / 3600) * Math.PI * 2;
        const x = Math.cos(ang) * belt.radius;
        const y = Math.sin(ang) * belt.radius * ORBIT_TILT;
        const arm = 0.016, waist = 0.003;
        const color = COMMODITY_COLOR[pocket.resource] ?? 0xffffff;
        const alpha = pocket.reserves !== null && pocket.reserves <= 0 ? 0.3 : 1;
        this.pocketGfx.poly([x, y - arm, x + waist, y, x, y + arm, x - waist, y]).fill({ color, alpha });
        this.pocketGfx.poly([x - arm, y, x, y - waist, x + arm, y, x, y + waist]).fill({ color, alpha });
        this.pocketGfx.poly([x, y - waist * 1.5, x + waist * 1.5, y, x, y + waist * 1.5, x - waist * 1.5, y]).fill({ color: 0xffffff, alpha });
      }
    }

    // Body fallback discs only (sprites cover the loaded case). Habitability
    // and deposits stay in the details — no rings or resource circles here.
    for (const node of this.nodes) {
      node.disc.clear();
      if (node.art) continue;
      const meta = KIND_META[node.kind === "moon" ? (node.moon!.deposits.length ? "ice" : "barren") : node.kind];
      circlePath(node.disc, 0, 0, node.radius, scale).fill({ color: meta.color, alpha: 1 });
      circlePath(node.disc, -node.radius * 0.28, -node.radius * 0.28, node.radius * 0.6, scale).fill({ color: meta.hi, alpha: 0.22 });
      circlePath(node.disc, 0, 0, node.radius, scale).stroke({ width: 0.0016, color: 0x0a0f1c, alpha: 0.7 });
    }
    // Rings are drawn at the displayed scale too.
    for (const node of this.nodes) {
      if (node.ringBack && node.ringFront) drawRings(node.ringBack, node.ringFront, node.radius, node.seed, scale);
    }
    this.coverSig = "";
  }

  /// The near zone draws no rings of its own. Only while the pointer rests on
  /// a world with your defenses does their reach show, as one dashed ring.
  private drawCover(): void {
    if (this.options.sceneryOnly) return;
    const cover = this.coverHover && !!this.picture?.platformCover;
    const sig = `${this.sceneScale.toFixed(2)}|${this.nearOuter.toFixed(3)}|${cover}`;
    if (sig === this.coverSig) return;
    this.coverSig = sig;
    const g = this.coverGfx;
    g.clear();
    if (!cover) return;
    const r = nearRadius(PLATFORM_COVER_SU, NEAR_INNER, this.nearOuter);
    dashedEllipse(g, r, r * ORBIT_TILT, 72).stroke({ width: 1.2 / this.sceneScale, color: 0xff7a6b, alpha: 0.5 });
  }

  /// Does this world carry the viewer's defenses (a station or platforms)?
  private defendsFrom(bodyId: string): boolean {
    if (!this.picture?.platformCover) return false;
    const body = this.wireBodies().find((b) => String(b.id) === bodyId);
    return (body?.structures?.orbital_defense ?? 0) > 0 || (body?.structures?.defense_platform ?? 0) > 0;
  }

  /// Fit the schematic to the viewport (plus the orrery camera's zoom) and
  /// re-pose. Called on setSystem, resize and camera changes; the static
  /// graphics redraw only when the displayed scale changed.
  layout(viewW: number, viewH: number, cameraRect: CameraRect = { x: 0, y: 0, w: viewW, h: viewH }): void {
    this.viewW = viewW;
    this.viewH = viewH;
    if (!viewW || !viewH) return;
    const usable = cameraRect.w > 0 && cameraRect.h > 0
      ? cameraRect
      : { x: 0, y: 0, w: viewW, h: viewH };
    this.cameraRect = { ...usable };
    const cx = usable.x + usable.w / 2;
    const cy = usable.y + usable.h / 2;
    this.center = { x: cx, y: cy };
    this.baseScale = Math.min(usable.w, usable.h) * SCENE_FIT;
    this.sceneScale = this.baseScale * this.zoom;
    this.nearOuter = nearOuterFor(usable.w / 2, usable.h / 2, this.baseScale);
    this.worldRoot.scale.set(this.sceneScale);
    this.worldRoot.position.set(cx - this.focus.x * this.sceneScale, cy - this.focus.y * this.sceneScale);
    this.starLayout = { x: this.worldRoot.position.x, y: this.worldRoot.position.y };

    if (this.vis && (this.gfxSystem !== this.vis.systemId || this.gfxScale !== this.sceneScale)) {
      this.redrawGfx(this.vis);
      this.gfxSystem = this.vis.systemId;
      this.gfxScale = this.sceneScale;
    }
    this.drawCover();

    if (!this.options.sceneryOnly) {
      // Backdrop dim (LOD separation from the galaxy starfield). No plate and
      // no boundary ring behind the orrery.
      this.vignette.clear();
      this.vignette.rect(0, 0, viewW, viewH).fill({ color: 0x05070d, alpha: 0.35 });
      // The headline sits just above the orrery (clear of any shell chrome
      // that overlaps the map's top edge), centred on the star.
      this.header.anchor.set(0.5, 0);
      this.header.position.set(cx, this.headlineY());
    }
    this.pose(this.poseTime);
  }

  /// The headline's y: above the near zone's top edge, inside the map.
  private headlineY(): number {
    const top = this.center.y - this.nearOuter * this.baseScale * ORBIT_TILT - 46;
    return Math.max(this.cameraRect.y + 10, top);
  }

  /// Place every world at `time` (observed sim seconds): orbital motion, depth
  /// order, lighting from the star, orbiting infrastructure, labels and hits.
  private pose(time: number): void {
    this.poseTime = time;
    if (!this.vis) return;
    const scale = this.sceneScale;
    const ox = this.worldRoot.position.x;
    const oy = this.worldRoot.position.y;
    for (const node of this.nodes) {
      let px: number;
      let py: number;
      let depth: number;
      if (!node.parent) {
        const a = orbitalAngle(node.baseAngle, planetPeriod(node.orbitRadius), time);
        px = Math.cos(a) * node.orbitRadius;
        py = Math.sin(a) * node.orbitRadius * ORBIT_TILT;
        depth = Math.sin(a) * node.orbitRadius;
      } else {
        const a = orbitalAngle(node.baseAngle, moonPeriod(node.moonIndex), time);
        px = node.parent.lx + Math.cos(a) * node.orbitRadius;
        py = node.parent.ly + Math.sin(a) * node.orbitRadius * ORBIT_TILT;
        depth = node.parent.z + Math.sin(a) * node.orbitRadius;
      }
      node.lx = px;
      node.ly = py;
      node.z = depth;
      node.group.position.set(px, py);
      node.group.zIndex = depth;
      const toStar = Math.atan2(-py, -px);
      if (node.art) node.art.rotation = towardStarRotation(toStar);
      if (node.shade) node.shade.rotation = toStar + Math.PI;
      node.lights.rotation = toStar + Math.PI;
      this.poseOrbitals(node, time, scale);
      const sr = Math.max(node.radius * scale, node.parent ? 9 : 13);
      node.hit.sx = ox + px * scale;
      node.hit.sy = oy + py * scale;
      node.hit.r = sr;
    }
    this.poseLights(scale);
    if (this.selectedId && this.selected) {
      const node = this.nodeById.get(this.selectedId);
      if (node) { this.selected.sx = node.hit.sx; this.selected.sy = node.hit.sy; this.selected.r = node.hit.r; }
    }
    if (!this.options.sceneryOnly) {
      this.placeLabels();
      this.drawChips();
    }
  }

  private poseOrbitals(node: BodyNode, time: number, scale: number): void {
    for (const { item, node: drawn } of node.orbitals.values()) {
      const a = item.phase + time * item.speed;
      const rx = node.radius * item.orbit;
      drawn.position.set(Math.cos(a) * rx, Math.sin(a) * rx * ORBIT_TILT);
      const front = Math.sin(a) >= 0;
      const parent = front ? node.orbitFront : node.orbitBack;
      if (drawn.parent !== parent) parent.addChild(drawn);
      if (drawn instanceof Sprite) {
        const tex = drawn.texture;
        drawn.scale.set(item.px / scale / Math.max(1, tex.width));
      } else {
        drawn.scale.set(1 / scale);
      }
      drawn.alpha = front ? 1 : 0.82;
    }
  }

  private poseLights(scale: number): void {
    if (this.options.sceneryOnly) return;
    const byId = new Map(this.wireBodies().map((b) => [String(b.id), b]));
    for (const node of this.nodes) {
      const population = byId.get(node.id)?.population ?? 0;
      const sig = `${population}|${scale.toFixed(1)}`;
      if (sig === node.lightsSig) continue;
      node.lightsSig = sig;
      drawCityLights(node.lights, node.radius, population, node.seed, scale);
    }
  }

  private placeLabels(): void {
    let li = 0;
    const placed: { x0: number; y0: number; x1: number; y1: number }[] = [];
    const overlaps = (x0: number, y0: number, x1: number, y1: number) =>
      placed.some((r) => x0 < r.x1 && x1 > r.x0 && y0 < r.y1 && y1 > r.y0);
    // Nearer worlds (lower on screen) claim their spot first; a label that
    // would collide tries above its world, then steps further down.
    const order = [...this.nodes].sort((a, b) => b.hit.sy - a.hit.sy);
    for (const node of order) {
      const deposits = node.planet?.deposits ?? node.moon!.deposits;
      const col = deposits.length ? COMMODITY_COLOR[deposits[0].resource] : 0x9fb0c8;
      if (node.parent && !deposits.length) continue;
      let t = this.labelPool[li];
      if (!t) { t = new Text({ text: "", style: LABEL_STYLE() }); t.anchor.set(0.5, 0); this.labels.addChild(t); this.labelPool[li] = t; }
      li++;
      t.visible = true;
      if (t.text !== node.detail.name) t.text = node.detail.name;
      t.style.fill = col;
      const ringPad = node.ringFront ? node.radius * 0.5 * this.sceneScale * ORBIT_TILT : 0;
      const r = Math.max(node.radius * this.sceneScale, 6) + ringPad;
      const w = (t.width || node.detail.name.length * 6.2) / 2;
      const h = t.height || 12;
      const sx = node.hit.sx;
      const sy = node.hit.sy;
      // Below the world first; then beside it (right, left); then above;
      // finally stacked further below.
      const candidates: [number, number][] = [
        [sx, sy + r + 3],
        [sx + r + 6 + w, sy - h / 2],
        [sx - r - 6 - w, sy - h / 2],
        [sx, sy - r - 3 - h],
        [sx, sy + r + 3 + h],
      ];
      let [x, y] = candidates[0];
      for (const [cx, cy] of candidates) {
        if (!overlaps(cx - w, cy, cx + w, cy + h)) { x = cx; y = cy; break; }
      }
      t.position.set(x, y);
      placed.push({ x0: x - w, y0: y, x1: x + w, y1: y + h });
    }
    for (let k = li; k < this.labelPool.length; k++) this.labelPool[k].visible = false;
    this.labelRects = placed;
  }

  /// One small status chip per world with something to say (owner only):
  /// construction underway, idle converters, food or storage trouble.
  private drawChips(): void {
    const g = this.chipGfx;
    g.clear();
    let used = 0;
    if (this.reportMine) {
      for (const node of this.nodes) {
        const status = this.statuses.get(node.bodyId);
        if (!status) continue;
        const parts: string[] = [];
        if (status.building) parts.push(`⚒ ${status.building}`);
        if (status.idle) parts.push(`${status.idle} idle`);
        if (status.warning) parts.push(status.warning);
        if (!parts.length) continue;
        const warn = !!status.warning || status.idle > 0;
        let t = this.chipTexts[used];
        if (!t) {
          t = new Text({ text: "", style: new TextStyle({ fontFamily: "ui-monospace, monospace", fontSize: 9, fill: 0xffffff }) });
          t.anchor.set(0.5);
          this.markers.addChild(t);
          this.chipTexts.push(t);
        }
        used++;
        const text = parts.join(" · ");
        if (t.text !== text) t.text = text;
        t.style.fill = warn ? 0xffc46b : 0x9fdcff;
        t.visible = true;
        const w = t.width + 10;
        const r = Math.max(node.radius * this.sceneScale, 6);
        // Under the world's label, else beside the world, clear of every label.
        const spots: [number, number][] = [
          [node.hit.sx, node.hit.sy + r + 23],
          [node.hit.sx + r + 8 + w / 2, node.hit.sy + 10],
          [node.hit.sx - r - 8 - w / 2, node.hit.sy + 10],
          [node.hit.sx, node.hit.sy + r + 37],
        ];
        const clear = ([x, y]: [number, number]) => !this.labelRects.some((b) =>
          x - w / 2 < b.x1 && x + w / 2 > b.x0 && y - 7 < b.y1 && y + 7 > b.y0);
        const [x, y] = spots.find(clear) ?? spots[0];
        t.position.set(x, y);
        this.labelRects.push({ x0: x - w / 2, y0: y - 7, x1: x + w / 2, y1: y + 7 });
        g.roundRect(x - w / 2, y - 7, w, 14, 6).fill({ color: 0x05070d, alpha: 0.78 })
          .stroke({ width: 1, color: warn ? 0xffc46b : 0x4fc3ff, alpha: 0.6 });
      }
    }
    for (let i = used; i < this.chipTexts.length; i++) this.chipTexts[i].visible = false;
  }

  private planetDetail(p: VisualPlanet): SystemBodyDetail {
    const meta = KIND_META[p.kind];
    return { id: p.id, name: p.name, kindLabel: meta.label, kindColor: meta.color, isMoon: false, habitable: p.habitable, deposits: p.deposits, description: meta.desc, icon: PLANET_ART_URL(p.kind) };
  }
  private beltDetail(belt: VisualBelt): SystemBodyDetail {
    const meta = BELT_META[belt.kind];
    return { id: belt.id, name: belt.name, kindLabel: meta.label, kindColor: meta.dust, isMoon: false, habitable: false, deposits: belt.deposits, description: meta.desc, icon: CHUNK_ART_URL };
  }

  /// §belts: the charted belt under a screen point — an annulus test in the
  /// tilted plane, a few pixels of slop around the band.
  private beltAt(sx: number, sy: number): VisualBelt | null {
    if (!this.vis || !this.sceneScale) return null;
    const ox = this.worldRoot.position.x;
    const oy = this.worldRoot.position.y;
    const dx = (sx - ox) / this.sceneScale;
    const dy = (sy - oy) / this.sceneScale / ORBIT_TILT;
    const d = Math.hypot(dx, dy);
    const slop = 6 / this.sceneScale;
    let best: VisualBelt | null = null;
    let bestGap = Infinity;
    for (const belt of this.vis.asteroidBelts) {
      const gap = Math.abs(d - belt.radius);
      if (gap <= belt.width / 2 + slop && gap < bestGap) { best = belt; bestGap = gap; }
    }
    return best;
  }

  private moonDetail(mn: VisualMoon): SystemBodyDetail {
    const icy = mn.deposits.some((d) => d.resource === "volatiles");
    return { id: mn.id, name: mn.name, kindLabel: icy ? "Icy moon" : "Moon", kindColor: KIND_META.ice.color, isMoon: true, habitable: false, deposits: mn.deposits, description: icy ? "A frozen moon — a natural store of volatiles for the system." : "A small natural satellite.", icon: MOON_ART_URL };
  }

  /// Hit-test a screen point against planets/moons; selects the nearest hit (for
  /// the selection ring) and returns its details, or clears selection + returns
  /// null on a miss. Opening the world is the ONLY planet interaction.
  pickBody(sx: number, sy: number): SystemBodyDetail | null {
    let best: BodyHit | null = null;
    let bestD = Infinity;
    for (const b of this.bodies) {
      const d = Math.hypot(b.sx - sx, b.sy - sy);
      if (d < b.r && d < bestD) { bestD = d; best = b; }
    }
    if (!best) {
      // §belts: worlds first, then the belt band behind them. A belt draws no
      // selection ring (it already is one).
      const belt = this.beltAt(sx, sy);
      this.selected = null;
      this.selectedId = belt ? belt.id : null;
      return belt ? this.beltDetail(belt) : null;
    }
    this.selected = { sx: best.sx, sy: best.sy, r: best.r };
    this.selectedId = best.detail.id;
    return best.detail;
  }

  /// Fleets, fights and wrecks first (the precise targets), then worlds.
  pick(sx: number, sy: number): SystemScenePick | null {
    if (this.options.sceneryOnly) return null;
    const traffic = this.traffic.pick(sx, sy);
    if (traffic) return traffic;
    const detail = this.pickBody(sx, sy);
    return detail ? { kind: "body", detail } : null;
  }

  /// Hover copy for the shell's tooltip (null over empty space).
  hover(sx: number, sy: number): string | null {
    if (this.options.sceneryOnly) return null;
    const traffic = this.traffic.hover(sx, sy);
    this.coverHover = false;
    if (traffic) return traffic;
    for (const b of this.bodies) {
      if (Math.hypot(b.sx - sx, b.sy - sy) >= b.r) continue;
      this.coverHover = this.defendsFrom(b.detail.id);
      return `${b.detail.name} · ${b.detail.kindLabel} · open world${this.coverHover ? " · its defenses reach the dashed ring" : ""}`;
    }
    const belt = this.beltAt(sx, sy);
    if (belt) {
      const ores = belt.deposits.map((d) => `${label(d.resource)} ×${d.richness.toFixed(2)}`).join(", ");
      const found = belt.pockets.length ? ` · ${belt.pockets.length} pocket${belt.pockets.length > 1 ? "s" : ""} found` : "";
      return `${belt.name} · ${BELT_META[belt.kind].label}${ores ? ` · ${ores}` : ""}${found} · open belt`;
    }
    return null;
  }

  /// The pointer left the map: drop any hover-only drawing.
  clearHover(): void {
    this.coverHover = false;
  }

  /// Selected hulls that may swap to the shared live 3D atlas this frame.
  modelCandidates(): TrafficModelCandidate[] {
    return this.options.sceneryOnly ? [] : this.traffic.modelCandidates();
  }

  /// §body-management: resolve a VISUAL body id to its detail.
  detailFor(bodyId: string): SystemBodyDetail | null {
    return this.nodeById.get(bodyId)?.detail ?? null;
  }

  /// A world's current screen position (for the world-rung handoff).
  bodyScreen(bodyId: string): { sx: number; sy: number; r: number } | null {
    const node = this.nodeById.get(bodyId);
    return node ? { sx: node.hit.sx, sy: node.hit.sy, r: node.hit.r } : null;
  }

  /// §body-management: pulse a body's sprite for a moment (the chip-click
  /// affordance). Also selects it, so the ring lingers after the pulse fades.
  pulseBody(bodyId: string): void {
    const node = this.nodeById.get(bodyId);
    if (!node) return;
    this.selected = { sx: node.hit.sx, sy: node.hit.sy, r: node.hit.r };
    this.selectedId = bodyId;
    this.pulse = { id: bodyId, until: performance.now() + 1400 };
  }

  // ---- The orrery camera: a short zoom toward a world, then the world rung --

  zoomFactor(): number {
    return this.zoom;
  }

  /// Zoom about a screen point (multiplicative), clamped to [1, max]. At 1 the
  /// camera recentres on the star. Returns the new target zoom.
  zoomAt(sx: number, sy: number, factor: number): number {
    const next = Math.max(1, Math.min(SYSTEM_ZOOM_MAX, this.zoomTarget * factor));
    const s0 = this.baseScale * this.zoomTarget;
    const under = { x: this.focusTarget.x + (sx - this.center.x) / s0, y: this.focusTarget.y + (sy - this.center.y) / s0 };
    const s1 = this.baseScale * next;
    this.focusTarget = next <= 1.0001 ? { x: 0, y: 0 } : { x: under.x - (sx - this.center.x) / s1, y: under.y - (sy - this.center.y) / s1 };
    this.zoomTarget = next;
    const near = next > 1.0001 ? this.bodyNear(sx, sy, 3) : null;
    const node = near ? this.nodeById.get(near.id) : undefined;
    this.follow = node ? { id: node.id, lx: node.lx, ly: node.ly } : null;
    return next;
  }

  /// Drag the orrery by a screen-pixel delta: the player takes the camera, so
  /// a followed world is let go. The focus stays inside the near zone's
  /// extent, keeping the star and its traffic within reach at any zoom.
  panBy(dx: number, dy: number): void {
    if (!this.sceneScale) return;
    this.follow = null;
    const ex = this.nearOuter;
    const ey = this.nearOuter * ORBIT_TILT;
    const hold = (v: Vec2): Vec2 => ({ x: Math.max(-ex, Math.min(ex, v.x)), y: Math.max(-ey, Math.min(ey, v.y)) });
    const sx = dx / this.sceneScale;
    const sy = dy / this.sceneScale;
    this.focus = hold({ x: this.focus.x - sx, y: this.focus.y - sy });
    this.focusTarget = hold({ x: this.focusTarget.x - sx, y: this.focusTarget.y - sy });
    this.layout(this.viewW, this.viewH, this.cameraRect);
  }

  /// Unzoomed and centred on the star (what Fit returns to).
  atRest(): boolean {
    return this.zoomTarget <= 1.0001 && Math.hypot(this.focusTarget.x, this.focusTarget.y) < 1e-6;
  }

  /// Frame a world at a zoom (the world-rung return lands here, then eases out).
  focusBody(bodyId: string, zoom: number): void {
    const node = this.nodeById.get(bodyId);
    if (!node) return;
    this.zoom = this.zoomTarget = Math.max(1, Math.min(SYSTEM_ZOOM_MAX, zoom));
    this.focus = { x: node.lx, y: node.ly };
    this.focusTarget = { ...this.focus };
    this.follow = { id: node.id, lx: node.lx, ly: node.ly };
    this.layout(this.viewW, this.viewH, this.cameraRect);
  }

  resetZoom(immediate = false): void {
    this.zoomTarget = 1;
    this.focusTarget = { x: 0, y: 0 };
    this.follow = null;
    if (immediate) {
      this.zoom = 1;
      this.focus = { x: 0, y: 0 };
      this.layout(this.viewW, this.viewH, this.cameraRect);
    }
  }

  /// The world nearest a screen point within reach (for the zoom handoff).
  bodyNear(sx: number, sy: number, slack = 2.2): SystemBodyDetail | null {
    let best: BodyHit | null = null;
    let bestD = Infinity;
    for (const b of this.bodies) {
      const d = Math.hypot(b.sx - sx, b.sy - sy);
      if (d < b.r * slack + 24 && d < bestD) { bestD = d; best = b; }
    }
    return best?.detail ?? null;
  }

  private tickCamera(dt: number): void {
    // Track the followed world's orbital motion so it stays where it was framed.
    if (this.follow && this.zoomTarget > 1.0001) {
      const node = this.nodeById.get(this.follow.id);
      if (node) {
        const dx = node.lx - this.follow.lx;
        const dy = node.ly - this.follow.ly;
        this.follow.lx = node.lx;
        this.follow.ly = node.ly;
        if (dx || dy) {
          this.focus = { x: this.focus.x + dx, y: this.focus.y + dy };
          this.focusTarget = { x: this.focusTarget.x + dx, y: this.focusTarget.y + dy };
          this.layout(this.viewW, this.viewH, this.cameraRect);
        }
      }
    } else if (this.zoomTarget <= 1.0001) {
      this.follow = null;
    }
    const dz = this.zoomTarget - this.zoom;
    const dfx = this.focusTarget.x - this.focus.x;
    const dfy = this.focusTarget.y - this.focus.y;
    if (Math.abs(dz) < 1e-4 && Math.abs(dfx) < 1e-5 && Math.abs(dfy) < 1e-5) {
      if (this.zoom !== this.zoomTarget) {
        this.zoom = this.zoomTarget;
        this.focus = { ...this.focusTarget };
        this.layout(this.viewW, this.viewH, this.cameraRect);
      }
      return;
    }
    const k = 1 - Math.exp(-12 * dt);
    this.zoom += dz * k;
    this.focus = { x: this.focus.x + dfx * k, y: this.focus.y + dfy * k };
    this.layout(this.viewW, this.viewH, this.cameraRect);
  }

  /// Draw the per-frame DYNAMIC layer: orbital motion at the observed time,
  /// the star's motion, the rival ring, selection rings and the traffic
  /// picture. `owner` comes from the caller's light-gated per-player view —
  /// identical fog to the galaxy map, so nothing hidden leaks here.
  update(owner: PlayerId | null, playerId: PlayerId | null, nowMs: number, observedTime?: number): void {
    if (this.options.sceneryOnly) return;
    const dt = this.lastFrameMs ? Math.min(0.1, Math.max(0, (nowMs - this.lastFrameMs) / 1000)) : 1 / 60;
    this.lastFrameMs = nowMs;
    if (!this.viewW) return;
    this.tickCamera(dt);
    if (observedTime !== undefined && Number.isFinite(observedTime)) this.pose(observedTime);
    else this.pose(this.poseTime);
    this.starFx.update(nowMs / 1000, this.starSprite);
    this.drawCover();

    const g = this.overlay;
    g.clear();
    const cx = this.starLayout.x;
    const cy = this.starLayout.y;
    const starR = 0.085 * this.sceneScale;
    // Your own star needs no ring while you are inside its system (the
    // workspace already says it is yours); a rival's star keeps its warning.
    const rival = owner !== null && owner !== playerId;
    if (rival) {
      const breath = 0.5 + 0.5 * Math.sin(nowMs / 1100);
      g.ellipse(cx, cy, starR + 8, (starR + 8) * 0.92).stroke({ width: 2, color: 0xff7a6b, alpha: 0.95 });
      g.ellipse(cx, cy, starR + 14 + breath * 3, (starR + 14 + breath * 3) * 0.92).stroke({ width: 1, color: 0xff7a6b, alpha: 0.25 + 0.2 * breath });
    }
    if (this.selected) {
      g.circle(this.selected.sx, this.selected.sy, this.selected.r + 4).stroke({ width: 1.5, color: 0xffffff, alpha: 0.85 });
    }
    // §body-management: the chip-click pulse — an expanding, fading ring.
    if (this.pulse) {
      const left = this.pulse.until - nowMs;
      const node = this.nodeById.get(this.pulse.id);
      if (left <= 0 || !node) {
        this.pulse = null;
      } else {
        const t = 1 - left / 1400;
        const wave = 0.5 + 0.5 * Math.sin(nowMs / 120);
        g.circle(node.hit.sx, node.hit.sy, node.hit.r + 6 + t * 14)
          .stroke({ width: 2, color: 0x4fc3ff, alpha: (1 - t) * (0.5 + 0.4 * wave) });
      }
    }

    // The light-age stamp: how old this picture of the star is at command.
    const age = this.picture?.age ?? 0;
    const name = (this.vis?.name ?? "").toUpperCase();
    // §charting: an unsurveyed system arrives with no roster at all.
    const uncharted = !!this.vis && this.vis.planets.length === 0 && this.vis.asteroidBelts.length === 0;
    const stamp = uncharted ? "UNCHARTED · SEND A SCOUT OR SURVEY PROBE" : age < 1 ? "LIVE PICTURE" : `PICTURE Δ${formatAge(age)} OLD`;
    const headerText = name ? `${name} · ${stamp}` : stamp;
    if (this.header.text !== headerText) this.header.text = headerText;
    this.header.style.fill = uncharted ? 0x9aa8b8 : age < 1 ? 0x7fb6d8 : 0xe2ad62;

    this.traffic.update({
      picture: this.picture,
      nowMs,
      dt,
      sceneScale: this.sceneScale,
      center: { x: cx, y: cy },
      nearInner: NEAR_INNER,
      nearOuter: this.nearOuter,
      starRadiusPx: starR,
      bounds: this.cameraRect,
      headline: { x: this.center.x, y: this.headlineY() + 16 },
      zoom: this.zoom,
      bodyAt: (id) => {
        const node = this.nodeById.get(String(id));
        return node ? { sx: node.hit.sx, sy: node.hit.sy, r: Math.max(node.radius * this.sceneScale, 6) } : null;
      },
      beltAt: (id) => this.vis?.asteroidBelts.find((b) => b.id === String(id))?.radius ?? null,
      shipArt: this.shipArt,
      battleIcon: this.battleIcon,
    });
  }
}

// Re-export for the summary panel / breadcrumb, if useful elsewhere.
export type { StarType };

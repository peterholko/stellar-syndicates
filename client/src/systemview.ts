// ============================================================================
// PLANET-LEVEL SYSTEM VIEW — presentation only (semantic-zoom level of detail).
// ============================================================================
//
// HARD BOUNDARY (read before extending this file):
//   This is a VIEW, not a second scale of gameplay. Planets / moons / belts are
//   PURELY VISUAL. They give the system's EXISTING, system-level deposits a
//   visual home, but the deposit still belongs to the STAR SYSTEM. There is:
//     • NO per-planet claiming, building, defending, or ownership.
//     • NO intra-system ship movement or intra-system combat.
//     • NO server-authoritative planet/moon entities.
//   Claims, production, stockpiles, combat, movement, fog — ALL stay resolved at
//   the star-system level exactly as on the galaxy map. If a future contributor
//   wants planet-level gameplay, that is a MAJOR sim/protocol decision (promoting
//   these to authoritative entities) — NOT something to slip in here.
//
//   §management-home does NOT soften this. The System View is now where an owned
//   system is RUN (the city-screen pattern), but every mechanic stays SYSTEM
//   level: the build menu it hosts issues the SAME system-level commands as the
//   old galaxy rail, buildings consume SYSTEM dev slots, and the structure
//   markers drawn at planets are DECORATIVE ANCHORS (like the deposit pips) —
//   a Habitat "on" the agri world is still the system's Habitat. There are no
//   per-planet slots, entities, or orders.
//
// DETERMINISM & FOG:
//   `buildVisualSystem` derives the schematic SHAPE deterministically from the
//   public system id (+ public geology), so every player sees the same geography
//   for a system — it is public, unchanging astronomy, safe to synthesize client
//   side. It must NEVER encode hidden/dynamic state. DYNAMIC state (ownership) is
//   NOT generated here: the scene receives it from the caller, sourced from the
//   SAME light-gated per-player view the galaxy map uses (state.systems), so a
//   rival's claim is only ever as fresh as the player's delayed observation.
// ============================================================================

import { Assets, Container, Graphics, Sprite, Text, TextStyle, Texture } from "pixi.js";
import type { Commodity, Deposit, PlayerId, SystemInfo } from "./protocol";
import { starAnchor, starTypeFor, starVisualRatio, type StarType } from "./stars";

// ---- Public presentation data model (client-side, non-authoritative) --------

export type PlanetKind =
  | "terrestrial"
  | "desert"
  | "ocean"
  | "ice"
  | "gas_giant"
  | "lava"
  | "barren";

export interface VisualMoon {
  id: string;
  name: string;
  orbitRadius: number; // normalized units, around its parent planet
  radius: number;
  angle: number;
  deposits: Deposit[];
}

export interface VisualPlanet {
  id: string;
  name: string;
  kind: PlanetKind;
  orbitRadius: number; // normalized units, around the star (0 = star, ~0.95 = edge)
  radius: number; // normalized units
  angle: number; // radians
  moons: VisualMoon[];
  deposits: Deposit[];
  habitable: boolean;
}

export interface VisualSystem {
  systemId: string;
  starType: string; // slug (stars.ts) — matches the galaxy-map icon
  planets: VisualPlanet[];
  asteroidBelts: { radius: number; width: number }[];
}

// A body the details panel can describe — planets AND moons flow through this so
// the caller (main.ts) needs no knowledge of the internal shape.
export interface SystemBodyDetail {
  /// The visual body's stable id (deterministic per system) — used by the caller
  /// to offer the developments that ANCHOR here (contextual build sugar). Purely
  /// a presentation handle; it never names a server entity.
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

// ---- Development → body VISUAL ANCHORS (§management-home) --------------------
// Each SYSTEM-level development draws its structure marker at a natural body —
// the same pattern as deposits getting a visual home. STILL PRESENTATION ONLY
// (the hard boundary above holds): the building belongs to the SYSTEM, consumes
// SYSTEM dev slots, and has no per-planet existence — the anchor merely decides
// WHERE its decorative marker renders. Anchors are DETERMINISTIC (same body
// every visit) because they derive from the deterministic visual system:
//   extractor  → the body carrying the system's RICHEST deposit (tie → the
//                first such body in inner→outer walk order)
//   refinery   → the first volatiles body (the icy-moon motif) → else the first
//                gas giant → else the outermost planet
//   habitat    → the first habitable (provisions) world → else the first
//                terrestrial/ocean world → else the innermost planet
//   shipyard   → an orbital yard over the PRIMARY (innermost) planet
//   depot      → an orbital warehouse, also at the primary planet
//   sensor     → a relay dish at the OUTERMOST planet (the far vantage)
//   defense    → a battle-station marker in close STAR orbit (bodyId null)
//   (interdictor: no such development exists yet — add its row when it does)
export type DevKey = "extractor" | "depot" | "shipyard" | "sensor_array" | "defense_platform" | "habitat" | "refinery";
/// The owner's built tiers, passed from the SAME owner-only view fields the rail
/// used (state.systems) — rivals' views carry 0s, so markers can never leak.
export interface DevTiers {
  extractor: number;
  depot: number;
  shipyard: number;
  sensor_array: number;
  defense_platform: number;
  habitat: number;
  refinery: number;
  habitat_fed: boolean;
  /// §build-progress: build keys currently UNDER CONSTRUCTION here (owner-only,
  /// straight from the view's queue). A development key hangs a construction
  /// glyph at that development's anchor body; any SHIP key hangs it at the
  /// shipyard anchor (Travian's hammer-on-the-plot). Decoration only.
  inProgress: string[];
}

/// bodyId per development (null = anchors at the star). Walk order is the
/// sorted (inner→outer) planet list, moons after their planet — fixed, so the
/// choice is stable across visits.
export function developmentAnchors(vis: VisualSystem): Record<DevKey, string | null> {
  interface Walk { id: string; kind: PlanetKind; habitable: boolean; deposits: Deposit[]; isMoon: boolean }
  const walk: Walk[] = [];
  for (const p of vis.planets) {
    walk.push({ id: p.id, kind: p.kind, habitable: p.habitable, deposits: p.deposits, isMoon: false });
    for (const mn of p.moons) walk.push({ id: mn.id, kind: "ice", habitable: false, deposits: mn.deposits, isMoon: true });
  }
  let richest: Walk | null = null;
  let richestVal = -1;
  for (const b of walk) {
    for (const d of b.deposits) {
      if (d.richness > richestVal) { richestVal = d.richness; richest = b; }
    }
  }
  const first = (pred: (b: Walk) => boolean): string | null => walk.find(pred)?.id ?? null;
  const primary = vis.planets[0]?.id ?? null; // planets are sorted inner→outer
  const outermost = vis.planets[vis.planets.length - 1]?.id ?? null;
  return {
    extractor: richest?.id ?? primary,
    refinery: first((b) => b.deposits.some((d) => d.resource === "volatiles"))
      ?? first((b) => b.kind === "gas_giant") ?? outermost,
    habitat: first((b) => b.habitable)
      ?? first((b) => !b.isMoon && (b.kind === "terrestrial" || b.kind === "ocean")) ?? primary,
    shipyard: primary,
    depot: primary,
    sensor_array: outermost,
    defense_platform: null, // the star — guarding the system core
  };
}

/// Which developments ANCHOR at a given body — the caller's contextual build
/// sugar ("this icy moon would host the Refinery"). Same system-level build
/// either way; this only picks a friendlier entry point.
export function anchorsAtBody(vis: VisualSystem, bodyId: string): DevKey[] {
  const a = developmentAnchors(vis);
  return (Object.keys(a) as DevKey[]).filter((k) => a[k] === bodyId);
}

// ---- Per-kind presentation (color + flavor). Descriptions are flavor only. ---

interface KindMeta {
  label: string;
  color: number; // base disc color
  hi: number; // sunlit highlight
  desc: string;
}
const KIND_META: Record<PlanetKind, KindMeta> = {
  terrestrial: { label: "Terrestrial world", color: 0x6c9a6a, hi: 0xb7d8a2, desc: "A rocky world with a thin atmosphere and mixed terrain." },
  desert: { label: "Desert world", color: 0xc39a5b, hi: 0xe8cf93, desc: "A parched, dune-wrapped world baked by its star." },
  ocean: { label: "Ocean world", color: 0x4a86c4, hi: 0x9fd0f2, desc: "A blue world sheathed in deep planetary seas." },
  ice: { label: "Ice world", color: 0x9fc4d6, hi: 0xe6f4fb, desc: "A frozen world of ice plains and volatile frosts." },
  gas_giant: { label: "Gas giant", color: 0xc98f52, hi: 0xecc78a, desc: "A banded giant — a natural well of fuel and volatiles." },
  lava: { label: "Lava world", color: 0xc2502f, hi: 0xffb14a, desc: "A molten world, crust cracked with glowing magma." },
  barren: { label: "Barren world", color: 0x8a8f9c, hi: 0xc2c8d4, desc: "An airless rock — cratered, still, and mineral-rich." },
};

// ---- Planet/moon/asteroid ART (§planet-art) -----------------------------------
// One icon per PlanetKind (filenames match the kind slugs exactly), plus a
// generic moon and an asteroid chunk — 256px RGBA, background-removed from the
// generated set (real alpha). The measured VISIBLE extent of each subject on
// its canvas, so sprites scale to exactly the radius the fallback circle used.
const PLANET_ART_URL = (kind: PlanetKind) => `/art/celestial_sprites/planets/${kind}.png`;
const MOON_ART_URL = "/art/celestial_sprites/planets/moon.png";
const CHUNK_ART_URL = "/art/celestial_sprites/planets/asteroid_belt_chunk.png";
/// Fraction of the canvas the planet disk fills (measured: 0.78–0.81 across kinds).
const PLANET_ART_FILL = 0.79;
const MOON_ART_FILL = 0.31;
const CHUNK_ART_FILL = 0.43;
/// Chunk sprites scattered along each belt ring (over the existing dust dots).
const BELT_CHUNKS = 22;

// Fallback star tints (used only when the star icon texture hasn't loaded).
const STAR_TINT: Record<string, number> = {
  red_dwarf: 0xff8a5c, yellow_star: 0xffe08a, white_star: 0xdfe8ff, blue_giant: 0x9fc4ff,
  red_giant: 0xff7a5c, white_dwarf: 0xcfe0ff, neutron_star: 0xbfe0ff, binary_star: 0xffd98a,
  black_hole: 0x40507a, magnetar: 0xc9a0ff,
};

// Deposit → the kinds of world it visually belongs on (VISUAL ASSOCIATION only —
// the deposit remains a SYSTEM-level entity). Mirrors the spec's mapping:
//   ore → rocky/barren · alloys → industrial (barren/desert) · fuel → gas giant
//   provisions → habitable (ocean/terrestrial) · volatiles → icy body.
const DEP_KINDS: Record<Commodity, PlanetKind[]> = {
  ore: ["barren", "desert", "terrestrial"],
  alloys: ["barren", "desert"],
  fuel: ["gas_giant"],
  provisions: ["ocean", "terrestrial"],
  volatiles: ["ice"],
};
const FILLER_KINDS: PlanetKind[] = ["terrestrial", "desert", "barren", "lava", "ice", "gas_giant", "ocean"];

const COMMODITY_COLOR: Record<Commodity, number> = {
  provisions: 0x7fdc8a, ore: 0xb0894f, fuel: 0xff9d5c, volatiles: 0x6bd0ff, alloys: 0xc99bff,
};

const ROMAN = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];

// ---- Deterministic RNG (public geography — same for every player) ------------

function hashId(id: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h >>> 0;
}
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const pick = <T>(rng: () => number, arr: T[]): T => arr[Math.floor(rng() * arr.length) % arr.length];

function radiusForKind(kind: PlanetKind, rng: () => number): number {
  if (kind === "gas_giant") return 0.052 + rng() * 0.024; // giants read clearly larger
  return 0.02 + rng() * 0.016;
}

// ---- The generator -----------------------------------------------------------
//
// Deterministic from the public system id + the VIEWER'S KNOWN geology
// (§explore — `deposits` comes from the light-gated view: the exact table when
// surveyed-or-owner, EMPTY when unsurveyed, in which case the schematic degrades
// to filler bodies with no resource pips). Produces the schematic SHAPE only.
// Ownership/dynamic state is added later by the scene — never here.
export function buildVisualSystem(sys: SystemInfo, sysDeposits: Deposit[]): VisualSystem {
  const rng = mulberry32(hashId(sys.id) ^ 0x5eed1a7); // salt: distinct from star assignment
  const st = starTypeFor(sys.id);

  const planets: VisualPlanet[] = [];
  const mkPlanet = (kind: PlanetKind, habitable: boolean, deposits: Deposit[]): VisualPlanet => {
    const p: VisualPlanet = { id: `${sys.id}:p${planets.length}`, name: "", kind, orbitRadius: 0, radius: 0, angle: 0, moons: [], deposits, habitable };
    planets.push(p);
    return p;
  };

  // 1. Give each KNOWN deposit a visual home body (association, not a new entity).
  //    Volatiles prefer an icy MOON of a gas giant (the "icy moon" motif) when the
  //    system also has a fuel/gas-giant body; otherwise an ice world.
  let gasGiant: VisualPlanet | null = null;
  const volatiles: Deposit[] = [];
  for (const d of sysDeposits) {
    if (d.resource === "volatiles") { volatiles.push(d); continue; }
    const kind = pick(rng, DEP_KINDS[d.resource]);
    const p = mkPlanet(kind, d.resource === "provisions", [d]);
    if (kind === "gas_giant") gasGiant = p;
  }
  for (const d of volatiles) {
    if (gasGiant) {
      gasGiant.moons.push({ id: `${gasGiant.id}:m${gasGiant.moons.length}`, name: "", orbitRadius: 0.03 + rng() * 0.02, radius: 0.008 + rng() * 0.004, angle: rng() * Math.PI * 2, deposits: [d] });
    } else {
      mkPlanet("ice", false, [d]);
    }
  }

  // 2. Fill out to 3–8 decorative planets (no deposits — purely for a sense of place).
  const target = Math.max(3, Math.min(8, sysDeposits.length + 1 + Math.floor(rng() * 3)));
  while (planets.length < target) mkPlanet(pick(rng, FILLER_KINDS), false, []);

  // 3. Deterministic shuffle, then lay planets on spaced orbits with slight jitter.
  for (let i = planets.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [planets[i], planets[j]] = [planets[j], planets[i]];
  }
  const n = planets.length;
  planets.forEach((p, i) => {
    const base = 0.2 + 0.75 * (n === 1 ? 0.5 : i / (n - 1));
    p.orbitRadius = Math.min(0.96, base + (rng() - 0.5) * 0.03);
    p.angle = rng() * Math.PI * 2;
    p.radius = radiusForKind(p.kind, rng);
    // Decorative moons (no deposits) — gas giants tend to have a couple.
    const moonCount = p.kind === "gas_giant" ? 1 + Math.floor(rng() * 2) : rng() < 0.22 ? 1 : 0;
    for (let m = 0; m < moonCount; m++) {
      p.moons.push({ id: `${p.id}:m${p.moons.length}`, name: "", orbitRadius: 0.028 + rng() * 0.02 + m * 0.014, radius: 0.006 + rng() * 0.004, angle: rng() * Math.PI * 2, deposits: [] });
    }
    // Name: system name + roman numeral (sorted by orbit for a natural inner→outer read).
  });
  planets.sort((a, b) => a.orbitRadius - b.orbitRadius);
  planets.forEach((p, i) => {
    p.name = `${sys.name} ${ROMAN[i] ?? i + 1}`;
    p.moons.forEach((mn, k) => (mn.name = `${p.name}${String.fromCharCode(97 + k)}`));
  });

  // 4. Asteroid belts — decorative rings placed in a gap (deposits stay on bodies).
  const belts: { radius: number; width: number }[] = [];
  const beltCount = rng() < 0.55 ? 1 : rng() < 0.2 ? 2 : 0;
  for (let b = 0; b < beltCount; b++) {
    belts.push({ radius: 0.3 + rng() * 0.55, width: 0.02 + rng() * 0.03 });
  }

  return { systemId: sys.id, starType: st.slug, planets, asteroidBelts: belts };
}

// ---- The scene ---------------------------------------------------------------
//
// Owns `root` (an independent scene container with its OWN schematic camera). The
// STATIC schematic (orbits, belts, planet/moon discs, star) is drawn ONCE per
// system into a scaled `worldRoot` and only re-laid-out on resize — never rebuilt
// per frame. DYNAMIC overlays (ownership ring, selection ring) live in a separate
// screen-space layer redrawn each frame. Labels are screen-space, positioned on
// layout. Nothing here is authoritative; nothing here can leak hidden state.

interface BodyHit {
  sx: number; sy: number; r: number; // screen-space center + hit radius
  detail: SystemBodyDetail;
  /// §management-home: true for a structure-marker hit target (resolves to its
  /// anchor body's detail) — filtered out and re-added on marker rebuilds.
  isMarker?: boolean;
}

const LABEL_STYLE = () => new TextStyle({ fill: 0x9fb0c8, fontFamily: "ui-monospace, monospace", fontSize: 10 });

// ---- Scale-aware circle tessellation (§orbit-ring fix) ------------------------
// The schematic's Graphics draw in NORMALIZED units (radii ~0.001–0.96) inside a
// worldRoot scaled to hundreds of pixels. Pixi picks a circle's segment count
// from the radius AT DRAW TIME, so a radius-0.5 "circle" gets a handful of
// segments and the scale magnifies it into a visible polygon. These helpers
// build the path manually with a segment count derived from the DISPLAYED pixel
// radius — smooth at any window size. (Graphics redraw on layout/scale changes
// only — the static-once philosophy holds; sprites scale cleanly and are
// untouched.)
const ringSegments = (pixelR: number): number => Math.max(64, Math.min(256, Math.ceil(pixelR * 0.75)));

/// A circle path (for .fill()/.stroke()) tessellated for its on-screen size.
function circlePath(g: Graphics, x: number, y: number, r: number, pixelScale: number): Graphics {
  const n = ringSegments(r * pixelScale);
  g.moveTo(x + r, y);
  for (let i = 1; i <= n; i++) {
    const a = (i / n) * Math.PI * 2;
    g.lineTo(x + Math.cos(a) * r, y + Math.sin(a) * r);
  }
  g.closePath();
  return g;
}

/// An axis-aligned ellipse path, same idea (the gas-band fallback).
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

export class SystemViewScene {
  readonly root = new Container();
  private vignette = new Graphics(); // screen-space backdrop
  private worldRoot = new Container(); // scaled schematic space (STATIC, cached)
  private orbitsGfx = new Graphics(); // orbit rings + belt dust (static)
  private beltChunks = new Container(); // asteroid-chunk sprites on the belts (static)
  private starLayer = new Container(); // the star (sprite or procedural fallback)
  private bodySprites = new Container(); // planet + moon ART sprites (static)
  private bodiesGfx = new Graphics(); // fallback discs + halos + resource pips (static)
  private starSprite: Sprite | null = null;
  private starGfx = new Graphics(); // procedural star fallback (static)
  private overlay = new Graphics(); // ownership + selection (screen-space, dynamic)
  private labels = new Container(); // screen-space labels
  private labelPool: Text[] = [];
  // §management-home: DECORATIVE structure markers at the developments' anchor
  // bodies (owner's systems only — see setDevelopments). Screen-space like the
  // labels; rebuilt on layout and when the tiers change (build completion), never
  // per frame. These are markers, NOT entities — the hard boundary above holds.
  private markers = new Container();
  private devTiers: DevTiers | null = null;
  private devSig = ""; // last-rendered tier signature — skip rebuilds when unchanged

  // Planet/moon/chunk textures (lazy, same idiom as render.ts loadArt — the
  // KIND_META tint circle stays as the fallback until each resolves).
  private kindTex = new Map<PlanetKind, Texture>();
  private moonTex: Texture | null = null;
  private chunkTex: Texture | null = null;
  private lastStarTex: Texture | null = null;

  private vis: VisualSystem | null = null;
  /// What the static Graphics were last drawn for — redraw only when the
  /// system or the layout scale changes (never per frame).
  private gfxSystem: string | null = null;
  private gfxScale = -1;
  private bodies: BodyHit[] = []; // screen-space hit targets (rebuilt on layout)
  /// Body screen positions keyed by visual body id — the markers' anchor lookup.
  private bodyScreen = new Map<string, { sx: number; sy: number; r: number; detail: SystemBodyDetail }>();
  private selected: { sx: number; sy: number; r: number } | null = null;
  private viewW = 0;
  private viewH = 0;
  private sceneScale = 1;

  constructor() {
    this.starLayer.addChild(this.starGfx);
    this.worldRoot.addChild(this.orbitsGfx, this.beltChunks, this.starLayer, this.bodySprites, this.bodiesGfx);
    this.root.addChild(this.vignette, this.worldRoot, this.markers, this.overlay, this.labels);
    this.root.visible = false;
    // Non-blocking: fallback circles render immediately; the scene rebuilds
    // once (cached thereafter) when the art lands.
    void this.loadArt();
  }

  /// Load the planet/moon/chunk textures (each independent; a missing icon
  /// simply leaves that body on its tint-circle fallback — noted, not fatal).
  private async loadArt(): Promise<void> {
    const load = async (url: string): Promise<Texture | null> => {
      try {
        return await Assets.load(url);
      } catch {
        return null;
      }
    };
    const kinds: PlanetKind[] = ["terrestrial", "desert", "ocean", "ice", "gas_giant", "lava", "barren"];
    const [moon, chunk, ...planets] = await Promise.all([
      load(MOON_ART_URL),
      load(CHUNK_ART_URL),
      ...kinds.map((k) => load(PLANET_ART_URL(k))),
    ]);
    this.moonTex = moon;
    this.chunkTex = chunk;
    kinds.forEach((k, i) => {
      const t = planets[i];
      if (t) this.kindTex.set(k, t);
    });
    // Art arrived after a system was already built → rebuild that one scene
    // (still cached; this happens at most once per session).
    if (this.vis) {
      const v = this.vis;
      this.vis = null;
      this.setSystem(v, this.lastStarTex);
    }
  }

  /// (Re)build the STATIC schematic for a system. No-op if already showing it.
  setSystem(vis: VisualSystem, starTex: Texture | null): void {
    if (this.vis?.systemId === vis.systemId) return;
    this.vis = vis;
    this.lastStarTex = starTex;
    this.selected = null;
    this.buildStatic(vis, starTex);
    this.layout(this.viewW, this.viewH);
  }

  currentId(): string | null {
    return this.vis?.systemId ?? null;
  }

  clearSelection(): void {
    this.selected = null;
  }

  /// §management-home: set (or clear) the developments to render as structure
  /// markers. The caller passes tiers ONLY for the viewer's OWN system — sourced
  /// from the same owner-only view fields the management panel reads, so a rival
  /// system always gets null and renders as pure scenery (fog holds). Cached:
  /// markers rebuild only when the signature changes (i.e. a build completed or
  /// the system/owner changed) or on layout.
  setDevelopments(tiers: DevTiers | null): void {
    const sig = tiers
      ? `${this.vis?.systemId ?? ""}|${tiers.extractor},${tiers.depot},${tiers.shipyard},${tiers.sensor_array},${tiers.defense_platform},${tiers.habitat},${tiers.refinery},${tiers.habitat_fed}|${tiers.inProgress.join(",")}`
      : "";
    if (sig === this.devSig) return; // same picture — keep the cached markers
    this.devTiers = tiers;
    this.devSig = sig;
    this.rebuildMarkers();
  }

  private buildStatic(vis: VisualSystem, starTex: Texture | null): void {
    const st = starTypeFor(vis.systemId);
    // SPRITES only — every vector circle is drawn in redrawGfx with a segment
    // count matched to the displayed pixel size (§orbit-ring fix). Sprites are
    // immune to the tessellation problem, so they stay built-once here.
    if (this.starSprite) { this.starSprite.destroy(); this.starSprite = null; }
    if (starTex) {
      const sp = new Sprite(starTex);
      const a = starAnchor(st);
      sp.anchor.set(a[0], a[1]);
      sp.scale.set(0.17 / (starVisualRatio(st) * starTex.width)); // visible star ≈ 0.17 units
      this.starSprite = sp;
      this.starLayer.addChild(sp); // above orbits + belt chunks, under bodies
    }

    // Belt ART chunks (an INDEPENDENT seeded stream from the dust dots, so both
    // stay deterministic and stable per system).
    this.beltChunks.removeChildren().forEach((c) => c.destroy());
    for (const belt of vis.asteroidBelts) {
      if (this.chunkTex) {
        const crng = mulberry32(hashId(vis.systemId + "chunks" + belt.radius.toFixed(3)));
        for (let i = 0; i < BELT_CHUNKS; i++) {
          const ang = (i / BELT_CHUNKS) * Math.PI * 2 + crng() * 0.25;
          const rr = belt.radius + (crng() - 0.5) * belt.width;
          const size = 0.008 + crng() * 0.008; // world radius of the chunk
          const sp = new Sprite(this.chunkTex);
          sp.anchor.set(0.5);
          sp.position.set(Math.cos(ang) * rr, Math.sin(ang) * rr);
          sp.scale.set((2 * size) / (CHUNK_ART_FILL * this.chunkTex.width));
          sp.rotation = crng() * Math.PI * 2;
          sp.alpha = 0.85 + crng() * 0.15;
          this.beltChunks.addChild(sp);
        }
      }
    }

    // Planet + moon ART sprites (the kind's icon when loaded — the tint-circle
    // fallback for any unloaded texture is drawn by redrawGfx).
    this.bodySprites.removeChildren().forEach((c) => c.destroy());
    this.forEachBody(vis, (x, y, r, _kind, _deposits, _habitable, tex, artFill) => {
      if (!tex) return;
      const sp = new Sprite(tex);
      sp.anchor.set(0.5);
      sp.position.set(x, y);
      sp.scale.set((2 * r) / (artFill * tex.width));
      this.bodySprites.addChild(sp);
    });

    // Force the vector pass on the next layout (system changed).
    this.gfxSystem = null;
  }

  /// Visit every planet + moon with its resolved position/radius/texture — the
  /// single geometry walk shared by the sprite build and the vector redraw, so
  /// the two passes can never disagree.
  private forEachBody(
    vis: VisualSystem,
    cb: (x: number, y: number, r: number, kind: PlanetKind, deposits: Deposit[], habitable: boolean, tex: Texture | null, artFill: number) => void,
  ): void {
    for (const p of vis.planets) {
      const px = Math.cos(p.angle) * p.orbitRadius;
      const py = Math.sin(p.angle) * p.orbitRadius;
      cb(px, py, p.radius, p.kind, p.deposits, p.habitable, this.kindTex.get(p.kind) ?? null, PLANET_ART_FILL);
      for (const mn of p.moons) {
        const mx = px + Math.cos(mn.angle) * mn.orbitRadius;
        const my = py + Math.sin(mn.angle) * mn.orbitRadius;
        // Moons: the moon icon (tiny); fallback = the old icy/rock speck. A
        // deposit-bearing moon keeps the same resource pip as a planet.
        cb(mx, my, mn.radius, mn.deposits.length ? "ice" : "barren", mn.deposits, false, this.moonTex, MOON_ART_FILL);
      }
    }
  }

  /// (Re)draw every VECTOR element of the schematic — star-fallback glow, orbit
  /// rings, belt dust, body fallback discs, habitable halos, deposit pips — in
  /// normalized coordinates but tessellated for the CURRENT displayed size
  /// (§orbit-ring fix: circlePath/ellipsePath). Called from layout() only when
  /// the system or the scene scale changed — never per frame, so the
  /// static-once caching philosophy holds.
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

    // Orbit rings — THE reported polygon bug — and the belts' dust-dot grit
    // (same deterministic stream as ever).
    this.orbitsGfx.clear();
    for (const p of vis.planets) {
      circlePath(this.orbitsGfx, 0, 0, p.orbitRadius, scale).stroke({ width: 0.0016, color: 0x2a3a58, alpha: 0.8 });
    }
    for (const belt of vis.asteroidBelts) {
      const dots = 90;
      const rng = mulberry32(hashId(vis.systemId + "belt" + belt.radius.toFixed(3)));
      for (let i = 0; i < dots; i++) {
        const ang = (i / dots) * Math.PI * 2 + rng() * 0.05;
        const rr = belt.radius + (rng() - 0.5) * belt.width;
        circlePath(this.orbitsGfx, Math.cos(ang) * rr, Math.sin(ang) * rr, 0.0016 + rng() * 0.0016, scale)
          .fill({ color: 0x6a7488, alpha: 0.5 });
      }
    }

    // Body fallback discs + the always-on overlays (halo, deposit pip).
    this.bodiesGfx.clear();
    const g = this.bodiesGfx;
    this.forEachBody(vis, (x, y, r, kind, deposits, habitable, tex) => {
      const meta = KIND_META[kind];
      if (!tex) {
        // Fallback disc (pre-art rendering, unchanged apart from tessellation).
        circlePath(g, x, y, r, scale).fill({ color: meta.color, alpha: 1 });
        // Sunlit highlight toward the star (origin) and a shaded far limb — cheap "3D".
        const toStar = Math.atan2(-y, -x);
        const hx = x + Math.cos(toStar) * r * 0.32;
        const hy = y + Math.sin(toStar) * r * 0.32;
        circlePath(g, hx, hy, r * 0.7, scale).fill({ color: meta.hi, alpha: 0.22 });
        circlePath(g, x - Math.cos(toStar) * r * 0.28, y - Math.sin(toStar) * r * 0.28, r * 0.92, scale).fill({ color: 0x02040a, alpha: 0.28 });
        if (kind === "gas_giant") {
          ellipsePath(g, x, y, r * 0.92, r * 0.34, scale).fill({ color: meta.hi, alpha: 0.14 }); // band hint
        }
        circlePath(g, x, y, r, scale).stroke({ width: 0.0016, color: 0x0a0f1c, alpha: 0.7 });
      }
      if (habitable) circlePath(g, x, y, r * 1.25, scale).stroke({ width: 0.0016, color: 0x7fdc8a, alpha: 0.5 }); // life halo
      // Resource pip — a small ring in the deposit's map color (VISUAL association).
      if (deposits.length) {
        const col = COMMODITY_COLOR[deposits[0].resource] ?? 0xffffff;
        circlePath(g, x + r * 0.9, y - r * 0.9, r * 0.42, scale)
          .fill({ color: col, alpha: 0.95 })
          .stroke({ width: 0.0012, color: 0x02040a, alpha: 0.7 });
      }
    });
  }

  /// Fit the schematic to the viewport and recompute screen-space hit targets +
  /// label positions. Called on setSystem and on resize (camera is a fixed fit —
  /// there is no intra-system pan/zoom; zoom-out is an EXIT gesture, handled by
  /// the caller). Static graphics don't change — only the worldRoot transform.
  layout(viewW: number, viewH: number): void {
    this.viewW = viewW;
    this.viewH = viewH;
    if (!viewW || !viewH) return;
    const cx = viewW / 2;
    const cy = viewH / 2;
    this.sceneScale = Math.min(viewW, viewH) * 0.42;
    this.worldRoot.position.set(cx, cy);
    this.worldRoot.scale.set(this.sceneScale);

    // Vector pass (§orbit-ring fix): tessellation depends on the DISPLAYED
    // size, so the static Graphics redraw when the system or scale changes —
    // exactly the layout events; never per frame.
    if (this.vis && (this.gfxSystem !== this.vis.systemId || this.gfxScale !== this.sceneScale)) {
      this.redrawGfx(this.vis);
      this.gfxSystem = this.vis.systemId;
      this.gfxScale = this.sceneScale;
    }

    // Backdrop vignette (subtle LOD separation from the galaxy).
    this.vignette.clear();
    this.vignette.rect(0, 0, viewW, viewH).fill({ color: 0x05070d, alpha: 0.35 });
    this.vignette.circle(cx, cy, Math.min(viewW, viewH) * 0.5).fill({ color: 0x0a1120, alpha: 0.35 });

    // Rebuild screen-space hit targets + labels from the cached schematic.
    this.bodies = [];
    this.bodyScreen.clear();
    let li = 0;
    const label = (text: string, sx: number, sy: number, screenR: number, col: number) => {
      let t = this.labelPool[li];
      if (!t) { t = new Text({ text: "", style: LABEL_STYLE() }); t.anchor.set(0.5, 0); this.labels.addChild(t); this.labelPool[li] = t; }
      t.visible = true; t.text = text; t.style.fill = col;
      t.position.set(sx, sy + screenR + 3);
      li++;
    };
    if (this.vis) {
      for (const p of this.vis.planets) {
        const px = cx + Math.cos(p.angle) * p.orbitRadius * this.sceneScale;
        const py = cy + Math.sin(p.angle) * p.orbitRadius * this.sceneScale;
        const screenR = Math.max(p.radius * this.sceneScale, 13);
        const pd = this.planetDetail(p);
        this.bodies.push({ sx: px, sy: py, r: screenR, detail: pd });
        this.bodyScreen.set(p.id, { sx: px, sy: py, r: screenR, detail: pd });
        const depCol = p.deposits.length ? COMMODITY_COLOR[p.deposits[0].resource] : 0x9fb0c8;
        label(p.name, px, py, screenR, depCol);
        for (const mn of p.moons) {
          const mx = px + Math.cos(mn.angle) * mn.orbitRadius * this.sceneScale;
          const my = py + Math.sin(mn.angle) * mn.orbitRadius * this.sceneScale;
          const mr = Math.max(mn.radius * this.sceneScale, 9);
          const md = this.moonDetail(mn);
          this.bodies.push({ sx: mx, sy: my, r: mr, detail: md });
          this.bodyScreen.set(mn.id, { sx: mx, sy: my, r: mr, detail: md });
          if (mn.deposits.length) label(mn.name, mx, my, mr, COMMODITY_COLOR[mn.deposits[0].resource]);
        }
      }
    }
    for (let k = li; k < this.labelPool.length; k++) this.labelPool[k].visible = false;
    // §management-home: marker positions derive from the body screen positions
    // just computed — rebuild them on every layout (still never per frame).
    this.rebuildMarkers();
  }

  private planetDetail(p: VisualPlanet): SystemBodyDetail {
    const meta = KIND_META[p.kind];
    return { id: p.id, name: p.name, kindLabel: meta.label, kindColor: meta.color, isMoon: false, habitable: p.habitable, deposits: p.deposits, description: meta.desc, icon: PLANET_ART_URL(p.kind) };
  }
  private moonDetail(mn: VisualMoon): SystemBodyDetail {
    const icy = mn.deposits.some((d) => d.resource === "volatiles");
    return { id: mn.id, name: mn.name, kindLabel: icy ? "Icy moon" : "Moon", kindColor: KIND_META.ice.color, isMoon: true, habitable: false, deposits: mn.deposits, description: icy ? "A frozen moon — a natural store of volatiles for the system." : "A small natural satellite.", icon: MOON_ART_URL };
  }

  // §management-home: draw the structure markers at their anchor bodies.
  // Screen-space (like the labels), rebuilt from the cached body positions on
  // layout / tier changes — never per frame. One small glyph per BUILT
  // development + a small ×N tier tag; stacked around the body when several
  // developments share an anchor (fixed dev order → deterministic placement).
  // Each marker also registers a hit target that resolves to its ANCHOR BODY,
  // so clicking a rig selects the body it sits on (focus, not a new action).
  private rebuildMarkers(): void {
    this.markers.removeChildren().forEach((c) => c.destroy());
    this.bodies = this.bodies.filter((b) => !b.isMarker);
    if (!this.vis || !this.devTiers || !this.viewW) return;
    const anchors = developmentAnchors(this.vis);
    const cx = this.viewW / 2;
    const cy = this.viewH / 2;
    // Fixed draw order → stable stacking offsets when anchors coincide.
    const ORDER: DevKey[] = ["extractor", "refinery", "habitat", "shipyard", "depot", "sensor_array", "defense_platform"];
    const perBody = new Map<string, number>(); // stack index per anchor body
    const tagStyle = () => new TextStyle({ fill: 0x9fb0c8, fontFamily: "ui-monospace, monospace", fontSize: 9 });
    for (const key of ORDER) {
      const tier = this.devTiers[key];
      if (!tier) continue;
      const bodyId = anchors[key];
      let mx: number;
      let my: number;
      let anchorDetail: SystemBodyDetail | null = null;
      if (bodyId) {
        const bs = this.bodyScreen.get(bodyId);
        if (!bs) continue;
        const slot = perBody.get(bodyId) ?? 0;
        perBody.set(bodyId, slot + 1);
        // Arc the markers around the body's upper rim, one slot per development.
        const ang = -Math.PI * 0.42 + slot * 0.7;
        mx = bs.sx + Math.cos(ang) * (bs.r + 11);
        my = bs.sy + Math.sin(ang) * (bs.r + 11);
        anchorDetail = bs.detail;
      } else {
        // Star anchor (defense platform): a station in close orbit.
        const slot = perBody.get("star") ?? 0;
        perBody.set("star", slot + 1);
        const ang = -Math.PI / 5 + slot * 0.6;
        const rr = 0.135 * this.sceneScale;
        mx = cx + Math.cos(ang) * rr;
        my = cy + Math.sin(ang) * rr;
      }
      const g = new Graphics();
      this.drawDevGlyph(g, key, this.devTiers);
      g.position.set(mx, my);
      this.markers.addChild(g);
      const tag = new Text({ text: `×${tier}`, style: tagStyle() });
      tag.anchor.set(0, 0.5);
      tag.position.set(mx + 7, my);
      this.markers.addChild(tag);
      if (anchorDetail) {
        // A marker click selects its anchor body (r ≈ glyph + tag extent).
        this.bodies.push({ sx: mx, sy: my, r: 11, detail: anchorDetail, isMarker: true });
      }
    }

    // §build-progress: CONSTRUCTION glyphs — Travian's hammer-on-the-plot. One
    // scaffold at each anchor with work underway: a development key at its own
    // anchor, any ship build at the shipyard anchor. Same stacking arc as the
    // built markers (they continue the perBody slots, so nothing overlaps);
    // cleared automatically when the job leaves the queue (tier-signature
    // change → rebuild). Decoration on the cached scene — never per frame.
    const SHIP_BUILD_KEYS = new Set(["convoy", "raider", "corvette", "colony", "scout"]);
    const sites = new Set<DevKey>();
    for (const k of this.devTiers.inProgress) {
      if (SHIP_BUILD_KEYS.has(k)) sites.add("shipyard");
      else if (k in anchors) sites.add(k as DevKey);
    }
    for (const key of sites) {
      const bodyId = anchors[key];
      let mx: number;
      let my: number;
      if (bodyId) {
        const bs = this.bodyScreen.get(bodyId);
        if (!bs) continue;
        const slot = perBody.get(bodyId) ?? 0;
        perBody.set(bodyId, slot + 1);
        const ang = -Math.PI * 0.42 + slot * 0.7;
        mx = bs.sx + Math.cos(ang) * (bs.r + 11);
        my = bs.sy + Math.sin(ang) * (bs.r + 11);
      } else {
        const slot = perBody.get("star") ?? 0;
        perBody.set("star", slot + 1);
        const ang = -Math.PI / 5 + slot * 0.6;
        const rr = 0.135 * this.sceneScale;
        mx = this.viewW / 2 + Math.cos(ang) * rr;
        my = this.viewH / 2 + Math.sin(ang) * rr;
      }
      // The scaffold: an amber dashed frame + crane jib — reads "under
      // construction" at a glance, distinct from every finished glyph.
      const g = new Graphics();
      const c = 0xffc46b;
      for (const [x1, y1, x2, y2] of [[-4, -4, -1, -4], [1, -4, 4, -4], [4, -4, 4, -1], [4, 1, 4, 4], [4, 4, 1, 4], [-1, 4, -4, 4], [-4, 4, -4, 1], [-4, -1, -4, -4]] as [number, number, number, number][]) {
        g.moveTo(x1, y1).lineTo(x2, y2).stroke({ width: 1.2, color: c, alpha: 0.9 });
      }
      g.moveTo(-2, 3).lineTo(2, -3).stroke({ width: 1.3, color: c, alpha: 0.95 }); // the jib
      g.circle(2, -3, 1).fill({ color: c, alpha: 0.95 }); // the hook
      g.position.set(mx, my);
      this.markers.addChild(g);
    }
  }

  /// The per-development glyph — small distinct silhouettes in the schematic
  /// style (bundled SVG icons stay in the DOM panels where they render crisply;
  /// at ~7px these hand shapes read better than rasterized icons).
  private drawDevGlyph(g: Graphics, key: DevKey, tiers: DevTiers): void {
    switch (key) {
      case "extractor": // amber mining rig: derrick triangle + drill line
        g.poly([-4, 2, 0, -5, 4, 2]).stroke({ width: 1.4, color: 0xd8a54a, alpha: 0.95 });
        g.moveTo(0, -5).lineTo(0, 4).stroke({ width: 1.2, color: 0xd8a54a, alpha: 0.8 });
        break;
      case "refinery": // orange stacked tanks + flare
        g.roundRect(-4.5, -2, 4, 6, 1).stroke({ width: 1.2, color: 0xff9d5c, alpha: 0.95 });
        g.roundRect(0.5, -4, 4, 8, 1).stroke({ width: 1.2, color: 0xff9d5c, alpha: 0.95 });
        g.circle(2.5, -5.5, 1.1).fill({ color: 0xffc46b, alpha: 0.9 }); // the flare
        break;
      case "habitat": { // green dome on a base line (+ warn tint when unfed)
        const col = tiers.habitat_fed ? 0x7fdc8a : 0xffc46b;
        g.arc(0, 2, 5, Math.PI, 0).stroke({ width: 1.4, color: col, alpha: 0.95 });
        g.moveTo(-5.5, 2).lineTo(5.5, 2).stroke({ width: 1.2, color: col, alpha: 0.8 });
        g.circle(0, -0.5, 1).fill({ color: col, alpha: 0.9 });
        break;
      }
      case "shipyard": // cyan orbital gantry: open bracket frame
        g.poly([-5, -4, -5, 4, -2, 4]).stroke({ width: 1.4, color: 0x4fc3ff, alpha: 0.95 });
        g.poly([5, -4, 5, 4, 2, 4]).stroke({ width: 1.4, color: 0x4fc3ff, alpha: 0.95 });
        g.moveTo(-5, -4).lineTo(5, -4).stroke({ width: 1.4, color: 0x4fc3ff, alpha: 0.95 });
        break;
      case "depot": // violet crate: square with a band
        g.rect(-4, -4, 8, 8).stroke({ width: 1.3, color: 0xc99bff, alpha: 0.95 });
        g.moveTo(-4, 0).lineTo(4, 0).stroke({ width: 1, color: 0xc99bff, alpha: 0.7 });
        break;
      case "sensor_array": // teal dish: arc + stem
        g.arc(0, -1, 4.5, Math.PI * 0.15, Math.PI * 0.85).stroke({ width: 1.4, color: 0x62d6c3, alpha: 0.95 });
        g.moveTo(0, 3).lineTo(0, 5.5).stroke({ width: 1.2, color: 0x62d6c3, alpha: 0.8 });
        g.circle(0, -2.5, 1).fill({ color: 0x62d6c3, alpha: 0.9 });
        break;
      case "defense_platform": // red battle station: ring + spokes
        g.circle(0, 0, 4.5).stroke({ width: 1.4, color: 0xff7a6b, alpha: 0.95 });
        for (const a of [0, Math.PI / 2, Math.PI, -Math.PI / 2]) {
          g.moveTo(Math.cos(a) * 4.5, Math.sin(a) * 4.5).lineTo(Math.cos(a) * 6.5, Math.sin(a) * 6.5).stroke({ width: 1.2, color: 0xff7a6b, alpha: 0.85 });
        }
        g.circle(0, 0, 1.4).fill({ color: 0xff7a6b, alpha: 0.9 });
        break;
    }
  }

  /// Hit-test a screen point against planets/moons; selects the nearest hit (for
  /// the selection ring) and returns its details, or clears selection + returns
  /// null on a miss. Opening the details panel is the ONLY planet interaction —
  /// there is no deeper camera level and no per-planet gameplay action.
  pickBody(sx: number, sy: number): SystemBodyDetail | null {
    let best: BodyHit | null = null;
    let bestD = Infinity;
    for (const b of this.bodies) {
      const d = Math.hypot(b.sx - sx, b.sy - sy);
      if (d < b.r && d < bestD) { bestD = d; best = b; }
    }
    if (!best) { this.selected = null; return null; }
    this.selected = { sx: best.sx, sy: best.sy, r: best.r };
    return best.detail;
  }

  /// Draw the per-frame DYNAMIC overlay: the star's ownership treatment (mine /
  /// rival / unclaimed) and the selection ring. `owner` comes from the caller's
  /// light-gated per-player view (state.systems) — identical fog to the galaxy
  /// map, so nothing hidden leaks here.
  update(owner: PlayerId | null, playerId: PlayerId | null, nowMs: number): void {
    const g = this.overlay;
    g.clear();
    if (!this.viewW) return;
    const cx = this.viewW / 2;
    const cy = this.viewH / 2;
    const starR = 0.085 * this.sceneScale;
    const mine = owner !== null && owner === playerId;
    const rival = owner !== null && !mine;
    if (mine) {
      g.circle(cx, cy, starR + 8).stroke({ width: 1.8, color: 0x4fc3ff, alpha: 0.95 });
      g.circle(cx, cy, starR + 14).stroke({ width: 1, color: 0x4fc3ff, alpha: 0.3 });
    } else if (rival) {
      const breath = 0.5 + 0.5 * Math.sin(nowMs / 1100);
      g.circle(cx, cy, starR + 8).stroke({ width: 2, color: 0xff7a6b, alpha: 0.95 });
      g.circle(cx, cy, starR + 14 + breath * 3).stroke({ width: 1, color: 0xff7a6b, alpha: 0.25 + 0.2 * breath });
    }
    if (this.selected) {
      g.circle(this.selected.sx, this.selected.sy, this.selected.r + 4).stroke({ width: 1.5, color: 0xffffff, alpha: 0.85 });
    }
  }
}

// Re-export for the summary panel / breadcrumb, if useful elsewhere.
export type { StarType };

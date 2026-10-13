// ============================================================================
// SYSTEM PICTURE — who is at a star, what is coming, what is being fought over.
// ============================================================================
//
// A pure derivation from the player's SERVED, light-gated picture (ghosts,
// battles, records, the system report). It adds no knowledge: every fleet here
// is a ghost the galaxy map could already draw, every battle a marker it could
// already show, every siege fact a field the server only sends to the two
// participants. What it decides is purely presentational:
//
//   * BERTH   — a fleet berthed at this system parks beside the world hosting
//               the facility it uses (systemlayout.berthAnchor).
//   * NEAR    — a fleet under way in the star's near zone keeps its true
//               bearing and distance, drawn to scale outside the orbits.
//   * CORDON  — a hostile fleet loitering at the system rings the world it
//               threatens (and draws the blockade there when one is served).
//   * BATTLE  — participants of a battle at this star face each other at the
//               battle's anchor.
//   * INBOUND — a contact beyond the near zone closing on the star becomes an
//               edge arrow with its served-motion ETA.
//   * OUTBOUND— a contact that has just left the near zone, by its served
//               motion, becomes an edge arrow pointing away. Own fleets name
//               their destination and arrival time from their flight plan.

import type {
  BattleRecordView, BattleView, GalaxyInfo, GhostView, GroundRecordView, PlayerId,
  SystemInfo, SystemStateView, Vec2,
} from "./protocol";
import { battleRecordMarkKey } from "./battlehistory";
import { blockadeBadge, siegeClock } from "./core/derive/blockade";
import {
  DOCK_RADIUS_SU, NEAR_ZONE_SU, PLATFORM_COVER_SU, berthAnchor, defendedWorld, pictureAge, siegeTarget,
  type AnchorBody,
} from "./systemlayout";

/** Where something is drawn: beside a world, in the near zone, or at the star. */
export type PictureAnchor =
  | { kind: "body"; body: number }
  | { kind: "near"; distance: number; bearing: number }
  | { kind: "star" };

export type FleetPlacement =
  | { mode: "berth"; body: number | null; slot: number; slots: number }
  | { mode: "cordon"; body: number | null; slot: number; slots: number }
  | { mode: "near"; distance: number; bearing: number }
  /** §belts: your station anchored at a belt, and your barges working it,
   *  sit on that belt's band. */
  | { mode: "belt"; body: number; slot: number; slots: number }
  | { mode: "battle"; battle: string; side: 0 | 1; slot: number; slots: number }
  | { mode: "inbound"; bearing: number; eta: number | null }
  /** `to`/`eta`: an own fleet's flight-plan destination and arrival, if known. */
  | { mode: "outbound"; bearing: number; eta: number | null; to: string | null };

export type FleetTone = "own" | "ally" | "rival" | "pirate" | "tca";

export interface PictureFleet {
  id: string;
  ghost: GhostView;
  placement: FleetPlacement;
  tone: FleetTone;
  hostile: boolean;
  /** Galaxy-space velocity angle of the served sighting; null when parked. */
  heading: number | null;
  selected: boolean;
}

export interface PictureBattle {
  id: string;
  live: boolean;
  anchor: PictureAnchor;
  own: boolean;
  /** 0..1 — how hard the arrived rounds say the fight is going. Cosmetic. */
  intensity: number;
  /** Light age of the sighting (s); null when only a record is known. */
  age: number | null;
  selected: boolean;
}

export interface PictureWreck {
  id: string;
  anchor: PictureAnchor;
  /** Deterministic cosmetic seed for the debris scatter. */
  seed: number;
  selected: boolean;
}

export interface PictureSiege {
  target: number | null;
  byMe: boolean;
  /** The stage as the headline reads it: CONTESTED, BLOCKADED, SIEGE n% …
   * (core/derive/blockade.ts), or null for a landing seen without one. */
  badge: string | null;
  /** 0..1 capture clock, or null while defenses still hold it off. */
  progress: number | null;
  /** 0..1 of the garrison pinned by bombardment. */
  suppression: number;
  landing: { id: string; marines: number; defenders: number } | null;
}

export interface SystemPicture {
  systemId: string;
  /** Light age of this system's picture as seen from command (s). */
  age: number;
  fleets: PictureFleet[];
  battles: PictureBattle[];
  wrecks: PictureWreck[];
  siege: PictureSiege | null;
  /** Own defenses stand here: pointing at their world shows their reach,
   * which is the near zone's edge. */
  platformCover: boolean;
}

export interface SystemPictureInput {
  system: SystemInfo;
  dynamic: SystemStateView | undefined;
  galaxy: Pick<GalaxyInfo, "c" | "siege_secs" | "sensor_range">;
  ghosts: readonly GhostView[];
  battles: readonly BattleView[];
  battleRecords: readonly BattleRecordView[];
  groundRecords: readonly GroundRecordView[];
  battleDismissed: ReadonlySet<string>;
  commandCenter: Vec2 | null;
  playerId: PlayerId | null;
  simTime: number;
  selectedShipId: string | null;
  selectedShipIds: ReadonlySet<string>;
  selectedBattleId: string | null;
  /** Named places (stars, the Market Hub) for own fleets' outbound arrows. */
  places?: readonly { name: string; pos: Vec2 }[];
}

/** A contact closer than this (and not berthed) is AT the star: cordon/orbit. */
const LOITER_SU = DOCK_RADIUS_SU * 1.6;
/** A battle inside the defenses' reach is fought in the star's orbit, at the
 * defended world; a fight tagged to this star beyond it sits at the zone's edge. */
const ORBITAL_BATTLE_SU = PLATFORM_COVER_SU;
/** Hulls a world always draws individually (the System View's smallest cap). */
const ALWAYS_DRAWN = 3;
/** Inbound arrows only for arrivals inside this window (sim s). */
const INBOUND_ETA_MAX_S = 45 * 60;
/** Outbound arrows only for fleets that left the near zone this recently (sim s). */
const OUTBOUND_SHOWN_S = 120;
/** A flight plan ending this close to a place ends there. */
const ARRIVE_SU = DOCK_RADIUS_SU * 2;

/** DockSite reaches the client as Display ("E29") while SystemInfo carries the
 * EntityId form ("29"); mirrors core/derive/fleet.ts dockedAtSystem. */
export function dockedHere(g: GhostView, systemId: string): boolean {
  return g.docked === systemId || g.docked === `E${systemId}`;
}

export function fleetTone(g: GhostView): FleetTone {
  if (g.own) return "own";
  if (g.tca) return "tca";
  if (g.pirate) return "pirate";
  if (g.ally) return "ally";
  return "rival";
}

function headingOf(g: GhostView): number | null {
  const speed = Math.hypot(g.vel.x, g.vel.y);
  return speed > 0.5 ? Math.atan2(g.vel.y, g.vel.x) : null;
}

function anchorBodies(dynamic: SystemStateView | undefined): AnchorBody[] {
  return (dynamic?.bodies ?? []).map((b) => ({
    id: b.id, parent: b.parent, habitable: b.habitable, size: b.size, kind: b.kind,
    population: b.population ?? 0, structures: b.structures ?? {},
  }));
}

interface FlightPlan { to: Vec2; eta: number | null }

/** Own fleets announce their own destination: the end of the flight plan and
 * the time to fly the rest of it. */
function ownPlan(g: GhostView): FlightPlan | null {
  const path = g.path;
  if (!path?.length) return null;
  let length = 0;
  let from = g.pos;
  for (const point of path) {
    length += Math.hypot(point.pos.x - from.x, point.pos.y - from.y);
    from = point.pos;
  }
  const speed = g.speed ?? Math.hypot(g.vel.x, g.vel.y);
  return { to: path[path.length - 1].pos, eta: speed > 0.5 ? length / speed : null };
}

function arrivesAt(plan: FlightPlan | null, pos: Vec2): boolean {
  return !!plan && Math.hypot(plan.to.x - pos.x, plan.to.y - pos.y) <= ARRIVE_SU;
}

/** The named place a flight plan ends at, if any. */
function placeAt(places: SystemPictureInput["places"], pos: Vec2): string | null {
  let best: string | null = null;
  let bestD = ARRIVE_SU;
  for (const place of places ?? []) {
    const d = Math.hypot(place.pos.x - pos.x, place.pos.y - pos.y);
    if (d <= bestD) { best = place.name; bestD = d; }
  }
  return best;
}

/** Seconds since a contact heading away from the star crossed the near zone's
 * edge, by its served motion; null unless it is clearly moving away. */
function leftSecondsAgo(g: GhostView, star: Vec2, distance: number): number | null {
  const speed = Math.hypot(g.vel.x, g.vel.y);
  if (speed < 1 || distance <= NEAR_ZONE_SU) return null;
  const radial = ((g.pos.x - star.x) * g.vel.x + (g.pos.y - star.y) * g.vel.y) / distance;
  if (radial < speed * 0.35) return null;
  return (distance - NEAR_ZONE_SU) / radial;
}

/** A contact's served motion closing on the star: its ETA, else null. */
function closingEta(g: GhostView, star: Vec2, distance: number): number | null {
  const speed = Math.hypot(g.vel.x, g.vel.y);
  if (speed < 1 || distance <= 0) return null;
  const radial = ((star.x - g.pos.x) * g.vel.x + (star.y - g.pos.y) * g.vel.y) / distance;
  if (radial < speed * 0.35) return null;
  return Math.max(0, distance - DOCK_RADIUS_SU) / radial;
}

function battleIntensity(rec: BattleRecordView | undefined): number {
  const last = rec?.rounds[rec.rounds.length - 1];
  if (!last) return 0.5;
  const kills = last.kills[0].length + last.kills[1].length;
  const dealt = last.dealt ? last.dealt[0] + last.dealt[1] : 0;
  return Math.min(1, 0.45 + (dealt > 0 ? 0.25 : 0) + Math.min(0.3, kills * 0.15));
}

/** Where a fight at (or near) this star is drawn. */
function battleAnchor(pos: Vec2, star: Vec2, bodies: AnchorBody[], besieged: boolean): PictureAnchor {
  const dx = pos.x - star.x;
  const dy = pos.y - star.y;
  const distance = Math.hypot(dx, dy);
  if (distance <= ORBITAL_BATTLE_SU) {
    const body = besieged ? siegeTarget(bodies) : defendedWorld(bodies);
    return body === null ? { kind: "star" } : { kind: "body", body };
  }
  return { kind: "near", distance, bearing: Math.atan2(dy, dx) };
}

function seedOf(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** §belts: the belt an own fleet is working at this star: its station's
 * anchorage, or the belt its barges are cutting (once at the star). The anchor
 * and the job are owner-only fields, so nothing here reaches a rival. */
function workingBelt(g: GhostView, systemId: string, star: Vec2): number | null {
  if (!g.own) return null;
  if (g.anchor && g.anchor.system === systemId && g.anchor.phase !== "approaching") return g.anchor.body;
  const job = g.industry;
  if (job?.kind === "mine" && job.system === systemId && job.status !== "travelling"
    && Math.hypot(g.pos.x - star.x, g.pos.y - star.y) <= BELT_WORK_SU) return job.body;
  return null;
}
/** Mirror of the sim's SKIM_RANGE_SU: barges work within this of the star. */
const BELT_WORK_SU = 1_500;

export function deriveSystemPicture(input: SystemPictureInput): SystemPicture {
  const { system, dynamic, galaxy } = input;
  const star = system.pos;
  const bodies = anchorBodies(dynamic);
  const blockade = dynamic?.blockade ?? null;
  const near = (pos: Vec2, radius: number) => Math.hypot(pos.x - star.x, pos.y - star.y) <= radius;

  // --- Battles: running markers first, then arrived-but-unfinished records.
  const recordById = new Map(input.battleRecords.map((rec) => [rec.id, rec]));
  const concluded = new Set(input.battleRecords.filter((rec) => rec.outcome !== null).map((rec) => rec.id));
  const atThisStar = (id: string, pos: Vec2) => {
    const rec = recordById.get(id);
    return rec?.system === system.id || (rec?.system == null && near(pos, NEAR_ZONE_SU));
  };
  const battles: PictureBattle[] = [];
  const liveIds = new Set<string>();
  for (const b of input.battles) {
    if (concluded.has(b.id) || !atThisStar(b.id, b.pos)) continue;
    liveIds.add(b.id);
    battles.push({
      id: b.id, live: true, anchor: battleAnchor(b.pos, star, bodies, !!blockade), own: b.own,
      intensity: battleIntensity(recordById.get(b.id)), age: b.age,
      selected: input.selectedBattleId === b.id,
    });
  }
  for (const rec of input.battleRecords) {
    if (rec.outcome !== null || liveIds.has(rec.id) || !atThisStar(rec.id, rec.pos)) continue;
    liveIds.add(rec.id);
    battles.push({
      id: rec.id, live: true, anchor: battleAnchor(rec.pos, star, bodies, !!blockade), own: rec.own_side !== null,
      intensity: battleIntensity(rec), age: null, selected: input.selectedBattleId === rec.id,
    });
  }

  // --- Wrecks: concluded fights whose record is still served and not dismissed.
  const wrecks: PictureWreck[] = input.battleRecords
    .filter((rec) => rec.outcome !== null && !liveIds.has(rec.id) && atThisStar(rec.id, rec.pos)
      && !input.battleDismissed.has(battleRecordMarkKey(rec.id)))
    .map((rec) => ({
      id: rec.id, anchor: battleAnchor(rec.pos, star, bodies, false), seed: seedOf(rec.id),
      selected: input.selectedBattleId === rec.id,
    }));

  // --- Fleets.
  const engagedIn = new Map<string, string>();
  const battleById = new Map(input.battles.map((b) => [b.id, b]));
  for (const pb of battles) {
    for (const id of battleById.get(pb.id)?.participants ?? []) engagedIn.set(id, pb.id);
  }
  const arrowRange = galaxy.sensor_range || 0;
  const fleets: PictureFleet[] = [];
  for (const g of input.ghosts) {
    const tone = fleetTone(g);
    const hostile = tone === "rival" || tone === "pirate";
    const selected = input.selectedShipId === g.id || input.selectedShipIds.has(g.id);
    const base = { id: g.id, ghost: g, tone, hostile, heading: headingOf(g), selected };
    const battle = engagedIn.get(g.id);
    if (battle) {
      const rec = recordById.get(battle);
      const side: 0 | 1 = rec
        ? (rec.sides[0].corp === g.owner ? 0 : 1)
        : (g.own ? 1 : 0);
      fleets.push({ ...base, placement: { mode: "battle", battle, side, slot: 0, slots: 1 } });
      continue;
    }
    if (dockedHere(g, system.id)) {
      fleets.push({ ...base, placement: { mode: "berth", body: berthAnchor(g.kind, bodies), slot: 0, slots: 1 } });
      continue;
    }
    const belt = workingBelt(g, system.id, star);
    if (belt !== null) {
      fleets.push({ ...base, placement: { mode: "belt", body: belt, slot: 0, slots: 1 } });
      continue;
    }
    if (g.docked) continue; // berthed elsewhere (another system or the Hub)
    const dx = g.pos.x - star.x;
    const dy = g.pos.y - star.y;
    const distance = Math.hypot(dx, dy);
    // Hostile fleets loitering at the star form the cordon; so do the
    // viewer's own fleets blockading it, rather than idling in the near zone.
    if (distance <= LOITER_SU && (hostile || (g.own && !!blockade?.by_me))) {
      fleets.push({ ...base, placement: { mode: "cordon", body: siegeTarget(bodies), slot: 0, slots: 1 } });
      continue;
    }
    const bearing = Math.atan2(dy, dx);
    if (distance <= NEAR_ZONE_SU) {
      fleets.push({ ...base, placement: { mode: "near", distance, bearing } });
      continue;
    }
    // Beyond the near zone only own fleets and hostiles get an edge arrow:
    // arriving soon, or just left. Everything else stays on the galaxy map.
    if (distance > arrowRange || !(g.own || hostile)) continue;
    const plan = g.own ? ownPlan(g) : null;
    const homeward = arrivesAt(plan, star);
    const eta = g.own ? (homeward ? plan?.eta ?? null : null) : closingEta(g, star, distance);
    if (eta !== null && eta <= INBOUND_ETA_MAX_S) {
      fleets.push({ ...base, placement: { mode: "inbound", bearing, eta } });
      continue;
    }
    const left = leftSecondsAgo(g, star, distance);
    if (left === null || left > OUTBOUND_SHOWN_S || homeward) continue;
    fleets.push({ ...base, placement: {
      mode: "outbound", bearing, eta: plan?.eta ?? null, to: plan ? placeAt(input.places, plan.to) : null,
    } });
  }

  // Stable slots per group: own fleets first, then heavier hulls, then id.
  const groups = new Map<string, PictureFleet[]>();
  for (const f of fleets) {
    const p = f.placement;
    const key = p.mode === "berth" || p.mode === "cordon" || p.mode === "belt" ? `${p.mode}:${p.body}`
      : p.mode === "battle" ? `battle:${p.battle}:${p.side}` : "";
    if (!key) continue;
    const list = groups.get(key) ?? [];
    list.push(f);
    groups.set(key, list);
  }
  for (const list of groups.values()) {
    list.sort((a, b) => Number(b.ghost.own) - Number(a.ghost.own)
      || a.ghost.kind.localeCompare(b.ghost.kind) || a.id.localeCompare(b.id));
    // Slots stay put when a fleet is selected; only one folded past the
    // smallest per-world cap is lifted into the last always-drawn slot.
    const chosen = list.findIndex((f) => f.selected);
    if (chosen > ALWAYS_DRAWN - 1) list.splice(ALWAYS_DRAWN - 1, 0, ...list.splice(chosen, 1));
    list.forEach((f, slot) => {
      const p = f.placement;
      if (p.mode === "berth" || p.mode === "cordon" || p.mode === "battle" || p.mode === "belt") {
        f.placement = { ...p, slot, slots: list.length };
      }
    });
  }

  // --- Siege: participant-only fields, never inferred for third parties. A
  // landing record is equally fog-filtered (a viewer without access gets none).
  const landingRecord = input.groundRecords.find((rec) => rec.system === system.id && rec.outcome === null);
  let siege: PictureSiege | null = null;
  if (blockade || landingRecord) {
    const latest = landingRecord?.rounds[landingRecord.rounds.length - 1];
    siege = {
      target: siegeTarget(bodies),
      byMe: blockade ? blockade.by_me : !!landingRecord?.attacking,
      badge: blockade ? blockadeBadge(blockade, input.simTime, galaxy.siege_secs) : null,
      progress: blockade && blockade.siege_since !== null
        ? (siegeClock(blockade, input.simTime, galaxy.siege_secs)?.pct ?? 0) / 100 : null,
      suppression: Math.max(0, Math.min(1, dynamic?.ground?.suppression ?? latest?.suppression ?? 0)),
      landing: landingRecord ? {
        id: landingRecord.id,
        marines: latest?.marines_frac ?? 1,
        defenders: latest?.defenders_frac ?? 1,
      } : null,
    };
  }

  const own = dynamic?.owner !== null && dynamic?.owner !== undefined && dynamic.owner === input.playerId;
  return {
    systemId: system.id,
    age: pictureAge(star, input.commandCenter, galaxy.c),
    fleets,
    battles,
    wrecks,
    siege,
    platformCover: own && (dynamic?.defense_tier ?? 0) > 0,
  };
}

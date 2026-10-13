// ============================================================================
// SYSTEM VIEW TRAFFIC — fleets, fights, wrecks and sieges drawn at a star.
// ============================================================================
//
// Draws a `SystemPicture` (systemtraffic.ts) in screen space over the orrery.
// Nothing here is a second gameplay scale: berths, the near zone and the
// battle sites are presentation of system-level facts from the served,
// light-gated picture. Motion between placements is an ease toward the newest
// served placement — never a prediction. Weapon effects at a battle are
// cosmetic and only run while a served battle (or its arriving record) says a
// fight is going on; their density follows the arrived rounds.

import { Container, Graphics, Sprite, Text, TextStyle, Texture } from "pixi.js";
import type { CountClass, GhostView, ShipKind } from "./protocol";
import { countClassLabel, fleetExactCount } from "./protocol";
import { ORBIT_TILT, formatAge, nearRadius, systemShipPx } from "./systemlayout";
import type { FleetTone, PictureAnchor, PictureFleet, SystemPicture } from "./systemtraffic";
import { seeded } from "./systembodyfx";

export interface ShipArt {
  tex: Texture;
  mult: number;
  anchor?: readonly [number, number];
}
export type ShipArtResolver = (ghost: GhostView) => ShipArt | null;

export interface TrafficFrame {
  picture: SystemPicture | null;
  nowMs: number;
  dt: number;
  /** Pixels per normalized orrery unit, and the star's screen position. */
  sceneScale: number;
  center: { x: number; y: number };
  nearInner: number;
  nearOuter: number;
  starRadiusPx: number;
  /** The visible map rectangle (labels stay inside it). */
  bounds: { x: number; y: number; w: number; h: number };
  /** Where the star's headline continues (siege status), centred. */
  headline: { x: number; y: number };
  /** The orrery camera's zoom (1 = fit): more hulls are drawn individually as it rises. */
  zoom: number;
  bodyAt(id: number): { sx: number; sy: number; r: number } | null;
  /** §belts: a charted belt's band radius (normalized orrery units). */
  beltAt?(id: number): number | null;
  shipArt: ShipArtResolver | null;
  battleIcon: (() => Texture | null) | null;
}

export type TrafficPick =
  | { kind: "fleet"; id: string }
  | { kind: "battle"; id: string }
  | { kind: "wreck"; id: string }
  | { kind: "landing"; id: string };

export interface TrafficModelCandidate {
  id: string;
  kind: ShipKind;
  x: number;
  y: number;
  canvasPx: number;
  angle: number;
  alpha: number;
  selected: boolean;
  escorts: number;
  sprite: Sprite;
  model: Sprite;
}

const TONE_COLOR: Record<FleetTone, number> = {
  own: 0x4fc3ff, ally: 0x74e08c, rival: 0xff7a6b, pirate: 0xe08a2c, tca: 0x7fa6c8,
};
const COL_THREAT = 0xff4d4d;
/** §belts: where on its band a belt's working fleets sit, and their spacing. */
const BELT_ARC_START = -2.2;
const BELT_ARC_STEP = 0.16;
const COL_SIEGE = 0xffb347;
const SHIP_ART_FACING = Math.PI / 2;

const KIND_LABEL: Record<ShipKind, string> = {
  tiny_freighter: "Tiny Freighter", small_freighter: "Small Freighter", convoy: "Medium Freighter",
  large_freighter: "Large Freighter", heavy_freighter: "Heavy Freighter", bulk_freighter: "Bulk Freighter",
  freighter: "Authority Freighter", raider: "Interceptor", corvette: "Corvette", colony: "Colony Ship",
  scout: "Scout", survey_probe: "Survey Probe", mining_barge: "Mining Barge", destroyer: "Destroyer", cruiser: "Cruiser", battleship: "Battleship",
  dreadnought: "Dreadnought", titan: "Titan", transport: "Troop Transport", builder: "Construction Ship",
  mining_belt_station: "Mining Belt Station",
};
/** A selected fleet is drawn larger so its live 3D hull (≥32px) qualifies. */
const SELECTED_MIN_PX = 36;
/** Individually drawn hulls per world by camera zoom; the rest fold into a
 * "+N" token (the rail's Fleets tab lists every berth). */
function berthCap(zoom: number): number {
  return zoom < 1.45 ? 3 : zoom < 2.1 ? 6 : 10;
}
function cordonCap(zoom: number): number {
  return zoom < 1.45 ? 5 : 9;
}
/** Berth/cordon rings are flattened a little less than the orbital plane. */
const RING_TILT = 0.74;
/** Slots avoid the wedge below a world, where its name and status sit. */
const LABEL_WEDGE = 1.15;
const EASE_RATE = 6.5;
const FADE_RATE = 3.2;

interface FleetMarker {
  root: Container;
  body: Graphics;
  sprite: Sprite;
  model: Sprite;
  chrome: Graphics;
  badge: Text;
  label: Text;
  x: number;
  y: number;
  alpha: number;
  angle: number;
  px: number;
  placed: boolean;
  present: boolean;
  fleet: PictureFleet | null;
  escorts: number;
}

interface SiteHit { kind: "battle" | "wreck" | "landing"; id: string; x: number; y: number; r: number; hover: string }

const mono = (size: number, fill: number, weight: "400" | "700" = "400") =>
  new TextStyle({ fontFamily: "ui-monospace, monospace", fontSize: size, fill, fontWeight: weight,
    dropShadow: { color: 0x02040a, alpha: 0.9, blur: 2, distance: 1 } });

function countBadge(ghost: GhostView): { text: string; estimate: boolean } {
  const exact = fleetExactCount(ghost);
  if (exact !== null) return { text: exact > 1 ? String(exact) : "", estimate: false };
  return ghost.count_class === "one" ? { text: "", estimate: false } : { text: countClassLabel(ghost.count_class as CountClass), estimate: true };
}

function escortsFor(ghost: GhostView): number {
  const exact = fleetExactCount(ghost);
  const n = exact ?? ({ one: 1, two_to_three: 2, four_to_seven: 5, eight_to_fifteen: 9, sixteen_to_thirty: 20, thirty_one_plus: 31 } as Record<string, number>)[ghost.count_class] ?? 1;
  return n <= 1 ? 0 : n <= 3 ? 2 : n <= 7 ? 4 : 6;
}

function hash(text: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

export class SystemTrafficLayer {
  readonly root = new Container();
  private readonly under = new Graphics(); // rings, arcs, debris — beneath hulls
  private readonly fleetsLayer = new Container();
  private readonly over = new Graphics(); // weapon bolts, flashes, chevrons — above hulls
  private readonly icons = new Container();
  private readonly texts = new Container();
  private markers = new Map<string, FleetMarker>();
  private textPool: Text[] = [];
  private textUsed = 0;
  private iconPool: Sprite[] = [];
  private iconUsed = 0;
  private sites: SiteHit[] = [];
  /// Fleets folded into a world's "+N" tokens this frame.
  private hiddenBerths = new Map<number, number>();
  private hiddenCordon = new Map<number, number>();
  private lastScale = 0;
  private lastCenter = { x: 0, y: 0 };

  constructor() {
    this.root.eventMode = "none";
    this.root.addChild(this.under, this.fleetsLayer, this.icons, this.over, this.texts);
  }

  /** Drop every marker (system changed or view left). */
  clear(): void {
    for (const marker of this.markers.values()) marker.root.destroy({ children: true });
    this.markers.clear();
    this.under.clear();
    this.over.clear();
    this.sites = [];
    for (const t of this.textPool) t.visible = false;
    for (const s of this.iconPool) s.visible = false;
  }

  update(frame: TrafficFrame): void {
    const { picture } = frame;
    this.under.clear();
    this.over.clear();
    this.sites = [];
    this.hiddenBerths.clear();
    this.hiddenCordon.clear();
    this.textUsed = 0;
    this.iconUsed = 0;
    const snap = Math.abs(frame.sceneScale - this.lastScale) > 0.5
      || Math.hypot(frame.center.x - this.lastCenter.x, frame.center.y - this.lastCenter.y) > 0.5;
    this.lastScale = frame.sceneScale;
    this.lastCenter = { ...frame.center };
    for (const marker of this.markers.values()) marker.present = false;

    if (picture) {
      const sitePositions = new Map<string, { x: number; y: number; u: { x: number; y: number } }>();
      for (const battle of picture.battles) sitePositions.set(battle.id, this.sitePosition(battle.anchor, frame, hash(battle.id)));
      this.drawSiege(picture, frame);
      for (const wreck of picture.wrecks) this.drawWreck(wreck.id, wreck.seed, wreck.selected, this.sitePosition(wreck.anchor, frame, wreck.seed), frame);
      for (const fleet of picture.fleets) this.placeFleet(fleet, frame, sitePositions, snap);
      for (const battle of picture.battles) this.drawBattle(battle, sitePositions.get(battle.id)!, picture, frame);
      for (const [bodyId, extra] of this.hiddenBerths) {
        const body = frame.bodyAt(bodyId);
        if (!body) continue;
        this.text(`+${extra} docked`, body.sx - body.r - 20, body.sy + 5, TONE_COLOR.own, 1, 9, true);
      }
      for (const [bodyId, extra] of this.hiddenCordon) {
        const body = frame.bodyAt(bodyId);
        if (!body) continue;
        this.text(`+${extra} hostile`, body.sx + body.r + 46, body.sy + 5, COL_THREAT, 0, 9, true);
      }
    }

    // Fade and retire markers that left the picture.
    const k = 1 - Math.exp(-FADE_RATE * frame.dt);
    for (const [id, marker] of this.markers) {
      const target = marker.present ? 1 : 0;
      marker.alpha += (target - marker.alpha) * k;
      if (!marker.present && marker.alpha < 0.03) {
        marker.root.destroy({ children: true });
        this.markers.delete(id);
        continue;
      }
      marker.root.alpha = marker.alpha;
    }
    for (let i = this.textUsed; i < this.textPool.length; i++) this.textPool[i].visible = false;
    for (let i = this.iconUsed; i < this.iconPool.length; i++) this.iconPool[i].visible = false;
  }

  /** Hit-test fleets first (the most precise target), then battle sites. */
  pick(sx: number, sy: number): TrafficPick | null {
    let best: TrafficPick | null = null;
    let bestD = Infinity;
    for (const [id, marker] of this.markers) {
      if (!marker.present || marker.alpha < 0.3) continue;
      const d = Math.hypot(marker.x - sx, marker.y - sy);
      const r = Math.max(13, marker.px * 0.55);
      if (d < r && d < bestD) { bestD = d; best = { kind: "fleet", id }; }
    }
    if (best) return best;
    for (const site of this.sites) {
      const d = Math.hypot(site.x - sx, site.y - sy);
      if (d < site.r && d < bestD) { bestD = d; best = { kind: site.kind, id: site.id }; }
    }
    return best;
  }

  hover(sx: number, sy: number): string | null {
    const pick = this.pick(sx, sy);
    if (!pick) return null;
    if (pick.kind === "fleet") {
      const fleet = this.markers.get(pick.id)?.fleet;
      if (!fleet) return null;
      return fleetHover(fleet);
    }
    return this.sites.find((site) => site.kind === pick.kind && site.id === pick.id)?.hover ?? null;
  }

  /** Selected hulls large enough to swap to the shared live 3D atlas. */
  modelCandidates(): TrafficModelCandidate[] {
    const out: TrafficModelCandidate[] = [];
    for (const [id, marker] of this.markers) {
      const fleet = marker.fleet;
      if (!fleet?.selected || !marker.present || !marker.sprite.visible || marker.px < 32) continue;
      if (fleet.ghost.pirate) continue; // pirate hull culture keeps its art
      out.push({
        id, kind: fleet.ghost.migrant ? "colony" : fleet.ghost.kind, x: marker.x, y: marker.y,
        canvasPx: marker.px, angle: marker.angle, alpha: marker.sprite.alpha, selected: true,
        escorts: marker.escorts, sprite: marker.sprite, model: marker.model,
      });
    }
    return out;
  }

  /** Screen position of a fleet marker (for signals/reticles). */
  fleetScreen(id: string): { x: number; y: number } | null {
    const marker = this.markers.get(id);
    return marker && marker.present ? { x: marker.x, y: marker.y } : null;
  }

  // --- Placement ------------------------------------------------------------

  private planeToScreen(r: number, bearing: number, frame: TrafficFrame): { x: number; y: number } {
    return {
      x: frame.center.x + Math.cos(bearing) * r * frame.sceneScale,
      y: frame.center.y + Math.sin(bearing) * r * frame.sceneScale * ORBIT_TILT,
    };
  }

  /** A battle/wreck site plus the outward unit vector its sides face along. */
  private sitePosition(anchor: PictureAnchor, frame: TrafficFrame, seed: number): { x: number; y: number; u: { x: number; y: number } } {
    if (anchor.kind === "body") {
      const body = frame.bodyAt(anchor.body);
      if (body) {
        const a = -2.25 + ((seed % 7) - 3) * 0.12;
        const d = body.r + 58;
        const u = { x: Math.cos(a), y: Math.sin(a) };
        return { x: body.sx + u.x * d, y: body.sy + u.y * d * 0.8, u };
      }
    }
    if (anchor.kind === "near") {
      const r = nearRadius(anchor.distance, frame.nearInner, frame.nearOuter);
      const p = this.planeToScreen(r, anchor.bearing, frame);
      return { ...p, u: { x: Math.cos(anchor.bearing), y: Math.sin(anchor.bearing) } };
    }
    const u = { x: Math.cos(-2.4), y: Math.sin(-2.4) };
    const d = frame.starRadiusPx + 36;
    return { x: frame.center.x + u.x * d, y: frame.center.y + u.y * d * 0.8, u };
  }

  private marker(id: string): FleetMarker {
    let marker = this.markers.get(id);
    if (!marker) {
      const root = new Container();
      const body = new Graphics();
      const sprite = new Sprite(Texture.EMPTY);
      sprite.anchor.set(0.5);
      sprite.visible = false;
      const model = new Sprite(Texture.EMPTY);
      model.anchor.set(0.5);
      model.visible = false;
      const chrome = new Graphics();
      const badge = new Text({ text: "", style: mono(9, 0xffffff, "700") });
      badge.anchor.set(0.5);
      const label = new Text({ text: "", style: mono(9, 0xff7a6b) });
      label.anchor.set(0, 0.5);
      root.addChild(body, sprite, model, chrome, badge, label);
      this.fleetsLayer.addChild(root);
      marker = { root, body, sprite, model, chrome, badge, label, x: 0, y: 0, alpha: 0, angle: 0, px: 0,
        placed: false, present: false, fleet: null, escorts: 0 };
      this.markers.set(id, marker);
    }
    return marker;
  }

  private placeFleet(
    fleet: PictureFleet,
    frame: TrafficFrame,
    sites: Map<string, { x: number; y: number; u: { x: number; y: number } }>,
    snap: boolean,
  ): void {
    const g = fleet.ghost;
    const p = fleet.placement;
    const t = frame.nowMs / 1000;
    if (p.mode === "berth" && p.slot >= berthCap(frame.zoom) && p.body !== null) {
      this.hiddenBerths.set(p.body, (this.hiddenBerths.get(p.body) ?? 0) + 1);
      return;
    }
    if (p.mode === "cordon" && p.slot >= cordonCap(frame.zoom) && p.body !== null) {
      this.hiddenCordon.set(p.body, (this.hiddenCordon.get(p.body) ?? 0) + 1);
      return;
    }
    let x = frame.center.x;
    let y = frame.center.y;
    let angle = -Math.PI / 2;
    let showArt = true;
    let labelText = "";
    let labelColor = TONE_COLOR[fleet.tone];

    if (p.mode === "berth" || p.mode === "cordon") {
      const body = p.body !== null ? frame.bodyAt(p.body) : null;
      const cx = body ? body.sx : frame.center.x;
      const cy = body ? body.sy : frame.center.y;
      const br = body ? body.r : frame.starRadiusPx;
      // Docked hulls hold an inner ring; a hostile cordon rings outside them.
      const perRing = p.mode === "cordon" ? cordonCap(frame.zoom) : berthCap(frame.zoom);
      const ring = Math.floor(p.slot / perRing);
      const inRing = Math.max(1, Math.min(perRing, p.slots - ring * perRing));
      const rx = br + (p.mode === "cordon" ? 40 : 17) + ring * 15;
      const ry = rx * RING_TILT;
      // Slots fan out from the top (the most important hull first), left and
      // right alternately, never into the label wedge beneath the world.
      const i = p.slot % perRing;
      const half = Math.ceil((inRing - 1) / 2);
      const step = half > 0 ? Math.min(0.85, (Math.PI - LABEL_WEDGE / 2) / half) : 0;
      const k = Math.ceil(i / 2);
      const sign = i % 2 === 1 ? -1 : 1;
      const sway = p.mode === "berth" ? Math.sin(t * 0.35 + p.slot) * 0.05 : 0;
      const a = -Math.PI / 2 + sign * k * step + sway + ring * 0.3;
      x = cx + Math.cos(a) * rx;
      y = cy + Math.sin(a) * ry;
      angle = p.mode === "berth"
        ? Math.atan2(-Math.cos(a) * ry, Math.sin(a) * rx) // tangent to the mooring ring
        : Math.atan2(cy - y, cx - x); // a cordon faces the world it threatens
    } else if (p.mode === "belt") {
      // §belts: a station and its barges hold a short arc of the band, the
      // station first; the arc's place is fixed per belt.
      const r = frame.beltAt?.(p.body) ?? frame.nearOuter;
      const bearing = BELT_ARC_START + p.body * 1.3 + (p.slot - (p.slots - 1) / 2) * BELT_ARC_STEP;
      ({ x, y } = this.planeToScreen(r, bearing, frame));
      angle = Math.atan2(Math.cos(bearing) * ORBIT_TILT, -Math.sin(bearing)); // along the band
    } else if (p.mode === "near") {
      const r = nearRadius(p.distance, frame.nearInner, frame.nearOuter);
      ({ x, y } = this.planeToScreen(r, p.bearing, frame));
      angle = fleet.heading !== null
        ? Math.atan2(Math.sin(fleet.heading) * ORBIT_TILT, Math.cos(fleet.heading))
        : Math.atan2(frame.center.y - y, frame.center.x - x);
      if (fleet.hostile) {
        labelText = `⚠ ${fleetName(g).toUpperCase()}`;
        labelColor = COL_THREAT;
      }
    } else if (p.mode === "battle") {
      const site = sites.get(p.battle);
      if (site) {
        const toward = p.side === 1 ? -1 : 1; // defenders hold the inner side
        const perp = { x: -site.u.y, y: site.u.x };
        const spread = (p.slot - (p.slots - 1) / 2) * 13;
        x = site.x + site.u.x * 17 * toward + perp.x * spread;
        y = site.y + site.u.y * 17 * toward * 0.8 + perp.y * spread;
        angle = Math.atan2(site.y - y, site.x - x);
      }
    } else if (p.mode === "inbound" || p.mode === "outbound") {
      const r = frame.nearOuter + 0.05;
      ({ x, y } = this.planeToScreen(r, p.bearing, frame));
      angle = p.mode === "inbound"
        ? Math.atan2(frame.center.y - y, frame.center.x - x)
        : Math.atan2(y - frame.center.y, x - frame.center.x);
      showArt = false;
      const eta = p.eta !== null ? ` · ${p.mode === "inbound" ? "ETA " : ""}${formatAge(p.eta)}` : "";
      const course = p.mode === "inbound" ? eta : p.to ? ` → ${p.to}${eta}` : " · leaving";
      labelText = `${fleet.hostile ? "⚠ " : ""}${fleetName(g)}${course}`;
      labelColor = fleet.hostile ? COL_THREAT : TONE_COLOR[fleet.tone];
    }

    const marker = this.marker(fleet.id);
    marker.present = true;
    marker.fleet = fleet;
    marker.escorts = escortsFor(g);
    const k = snap || !marker.placed ? 1 : 1 - Math.exp(-EASE_RATE * frame.dt);
    marker.x += (x - marker.x) * k;
    marker.y += (y - marker.y) * k;
    if (!marker.placed) { marker.x = x; marker.y = y; }
    marker.placed = true;
    marker.angle = angle;
    marker.root.position.set(marker.x, marker.y);

    const art = showArt ? frame.shipArt?.(g) ?? null : null;
    const base = systemShipPx(g.kind);
    const size = fleet.selected ? Math.max(base * 1.5, SELECTED_MIN_PX) : base;
    const fade = Math.min(1, g.age / 45);
    const alpha = g.own ? Math.max(0.7, 0.98 - 0.3 * fade) : Math.max(0.5, 0.95 - 0.45 * fade);
    marker.body.clear();
    if (art) {
      marker.sprite.visible = true;
      if (marker.sprite.texture !== art.tex) marker.sprite.texture = art.tex;
      marker.sprite.anchor.set(art.anchor?.[0] ?? 0.5, art.anchor?.[1] ?? 0.5);
      marker.px = size * art.mult;
      marker.sprite.scale.set(marker.px / art.tex.width);
      marker.sprite.rotation = angle + SHIP_ART_FACING;
      marker.sprite.alpha = alpha;
    } else {
      marker.sprite.visible = false;
      marker.model.visible = false;
      marker.px = showArt ? size : 14;
      const color = TONE_COLOR[fleet.tone];
      if (showArt) {
        const len = size * 0.42;
        marker.body.poly([len, 0, -len * 0.7, -len * 0.45, -len * 0.7, len * 0.45]).fill({ color: 0xc9d6e8, alpha });
      } else {
        // Edge arrow: points at the star (inbound) or away from it (outbound).
        marker.body.poly([7, 0, -5, -6, -2, 0, -5, 6]).fill({ color, alpha: 0.95 });
      }
      marker.body.rotation = angle;
    }

    // Ownership pip, count badge, selection ring — the galaxy map's grammar.
    const chrome = marker.chrome;
    chrome.clear();
    const half = marker.px / 2;
    const pipColor = TONE_COLOR[fleet.tone];
    const pipR = 3.2;
    const pipY = -(Math.max(8, half * 0.75) + 4);
    const diamond = (cy: number, rr: number) => [0, cy - rr, rr, cy, 0, cy + rr, -rr, cy];
    chrome.poly(diamond(pipY, pipR + 1.2)).fill({ color: 0x05070d, alpha: 0.75 });
    chrome.poly(diamond(pipY, pipR)).fill({ color: pipColor, alpha: 0.95 });
    if (fleet.selected) {
      chrome.circle(0, 0, Math.max(11, half * 0.8) + 4).stroke({ width: 1.5, color: 0xffffff, alpha: 0.85 });
    }
    if (fleet.hostile && p.mode === "near") {
      const pulse = 0.5 + 0.5 * Math.sin(t * 4.3);
      chrome.circle(0, 0, Math.max(10, half * 0.7) + 3 + pulse * 4).stroke({ width: 1.2, color: COL_THREAT, alpha: 0.3 + 0.35 * pulse });
    }
    const badge = countBadge(g);
    const arrow = p.mode === "inbound" || p.mode === "outbound";
    marker.badge.visible = !!badge.text && !arrow;
    if (marker.badge.visible) {
      const bx = Math.max(8, half * 0.62);
      const by = Math.max(6, half * 0.5);
      const w = Math.max(12, badge.text.length * 6 + 6);
      chrome.roundRect(bx - w / 2, by - 6, w, 12, 5).fill({ color: 0x05070d, alpha: 0.82 })
        .stroke({ width: 1, color: pipColor, alpha: badge.estimate ? 0.5 : 0.9 });
      marker.badge.text = badge.text;
      marker.badge.style.fill = badge.estimate ? 0x9fb2c9 : 0xffffff;
      marker.badge.position.set(bx, by);
    }
    if (!labelText && fleet.selected) {
      labelText = fleetName(g).toUpperCase();
      labelColor = 0xffffff;
    }
    if (labelText && g.age >= 1 && !arrow) labelText += `  Δ${formatAge(g.age)}`;
    marker.label.visible = !!labelText;
    if (labelText) {
      marker.label.text = labelText;
      marker.label.style.fill = labelColor;
      // Keep the label inside the visible map: flip it left near the edge.
      const offset = Math.max(9, half * 0.7) + 5;
      const right = frame.bounds.x + frame.bounds.w - 6;
      const flip = marker.x + offset + marker.label.width > right;
      marker.label.anchor.set(flip ? 1 : 0, 0.5);
      marker.label.position.set(flip ? -offset : offset, -Math.max(7, half * 0.55));
    }
  }

  // --- Battles, wrecks, sieges -------------------------------------------

  private text(content: string, x: number, y: number, color: number, anchorX = 0.5, size = 9, bold = false): Text {
    let t = this.textPool[this.textUsed];
    if (!t) {
      t = new Text({ text: "", style: mono(size, color, bold ? "700" : "400") });
      this.texts.addChild(t);
      this.textPool.push(t);
    }
    this.textUsed++;
    t.visible = true;
    t.text = content;
    t.style.fill = color;
    t.style.fontSize = size;
    t.style.fontWeight = bold ? "700" : "400";
    t.anchor.set(anchorX, 1);
    t.position.set(x, y);
    t.alpha = 1;
    return t;
  }

  private icon(texture: Texture, x: number, y: number, px: number, alpha: number): void {
    let s = this.iconPool[this.iconUsed];
    if (!s) {
      s = new Sprite(texture);
      s.anchor.set(0.5);
      this.icons.addChild(s);
      this.iconPool.push(s);
    }
    this.iconUsed++;
    s.visible = true;
    s.texture = texture;
    s.position.set(x, y);
    s.scale.set(px / Math.max(1, texture.width));
    s.alpha = alpha;
  }

  private drawBattle(
    battle: SystemPicture["battles"][number],
    site: { x: number; y: number; u: { x: number; y: number } },
    picture: SystemPicture,
    frame: TrafficFrame,
  ): void {
    const now = frame.nowMs;
    const pulse = 0.5 - 0.5 * Math.cos(now / 1400 * Math.PI * 2);
    const g = this.under;
    g.circle(site.x, site.y, 24 + pulse * 5).fill({ color: COL_THREAT, alpha: 0.05 + 0.05 * pulse });
    g.circle(site.x, site.y, 24 + pulse * 5).stroke({ width: 1.4, color: COL_THREAT, alpha: 0.35 + 0.35 * pulse });
    if (battle.selected) this.bracket(site.x, site.y, 30, COL_THREAT);

    // Weapon fire between the two sides' served participants. Cosmetic: the
    // schedule is seeded per time slot; density follows the arrived rounds.
    const sides: [FleetMarker[], FleetMarker[]] = [[], []];
    for (const fleet of picture.fleets) {
      const p = fleet.placement;
      if (p.mode !== "battle" || p.battle !== battle.id) continue;
      const marker = this.markers.get(fleet.id);
      if (marker?.present) sides[p.side].push(marker);
    }
    const slotMs = 150;
    const slot = Math.floor(now / slotMs);
    const phase = (now % slotMs) / slotMs;
    const shots = 1 + Math.round(battle.intensity * 4);
    const rnd = seeded(hash(battle.id) ^ slot);
    const point = (side: number): { x: number; y: number } => {
      const list = sides[side];
      if (list.length) {
        const m = list[Math.floor(rnd() * list.length) % list.length];
        return { x: m.x, y: m.y };
      }
      const toward = side === 1 ? -1 : 1;
      return { x: site.x + site.u.x * 17 * toward + (rnd() - 0.5) * 16, y: site.y + site.u.y * 14 * toward + (rnd() - 0.5) * 10 };
    };
    for (let i = 0; i < shots; i++) {
      const from = rnd() < 0.5 ? 0 : 1;
      const a = point(from);
      const b = point(1 - from);
      const color = from === 0 ? 0xff9a7a : 0x8fd8ff;
      const head = Math.min(1, phase * 1.4);
      const tail = Math.max(0, head - 0.35);
      this.over.moveTo(a.x + (b.x - a.x) * tail, a.y + (b.y - a.y) * tail)
        .lineTo(a.x + (b.x - a.x) * head, a.y + (b.y - a.y) * head)
        .stroke({ width: 1.6, color, alpha: 0.9 });
      if (phase > 0.72) {
        this.over.circle(b.x + (rnd() - 0.5) * 4, b.y + (rnd() - 0.5) * 4, 2 + (phase - 0.72) * 9)
          .fill({ color: 0xfff1c9, alpha: (1 - phase) * 1.8 });
      }
    }
    const emblem = frame.battleIcon?.() ?? null;
    if (emblem) this.icon(emblem, site.x, site.y - 30, 22, 0.65 + 0.35 * pulse);
    const age = battle.age !== null && battle.age >= 1 ? ` · Δ${formatAge(battle.age)}` : "";
    this.text(`BATTLE${age}`, site.x, site.y - 42, COL_THREAT, 0.5, 9, true);
    this.sites.push({ kind: "battle", id: battle.id, x: site.x, y: site.y, r: 26,
      hover: `Battle in progress${age} · open the arrived combat picture` });
  }

  private drawWreck(id: string, seed: number, selected: boolean, site: { x: number; y: number }, frame: TrafficFrame): void {
    const g = this.under;
    const t = frame.nowMs / 1000;
    const rnd = seeded(seed);
    g.ellipse(site.x, site.y, 20, 20 * ORBIT_TILT).fill({ color: 0x9fb0c8, alpha: 0.05 });
    for (let i = 0; i < 11; i++) {
      const a = rnd() * Math.PI * 2 + t * (0.03 + rnd() * 0.04);
      const d = 4 + rnd() * 15;
      const x = site.x + Math.cos(a) * d;
      const y = site.y + Math.sin(a) * d * ORBIT_TILT;
      const s = 1.4 + rnd() * 2.6;
      const spin = rnd() * Math.PI * 2 + t * (rnd() - 0.5) * 0.8;
      const c = Math.cos(spin), sn = Math.sin(spin);
      g.poly([x + c * s, y + sn * s, x - sn * s * 0.6, y + c * s * 0.6, x - c * s * 0.8, y - sn * s * 0.8])
        .fill({ color: rnd() < 0.25 ? 0xb07a4a : 0x8a93a3, alpha: 0.85 });
    }
    if (selected) this.bracket(site.x, site.y, 24, 0xffffff);
    this.sites.push({ kind: "wreck", id, x: site.x, y: site.y, r: 20, hover: "Wreck field · open the battle replay" });
  }

  private drawSiege(picture: SystemPicture, frame: TrafficFrame): void {
    const siege = picture.siege;
    if (!siege || siege.target === null) return;
    const body = frame.bodyAt(siege.target);
    if (!body) return;
    const now = frame.nowMs;
    const pulse = 0.5 + 0.5 * Math.sin(now / 500);
    const g = this.under;
    const rx = body.r + 40;
    const ry = rx * RING_TILT;
    // The cordon: a dashed ring around the besieged world, on the hostile ring.
    const seg = 28;
    for (let i = 0; i < seg; i += 2) {
      const a0 = (i / seg) * Math.PI * 2 + now / 9000;
      const a1 = ((i + 1) / seg) * Math.PI * 2 + now / 9000;
      g.moveTo(body.sx + Math.cos(a0) * rx, body.sy + Math.sin(a0) * ry)
        .lineTo(body.sx + Math.cos(a1) * rx, body.sy + Math.sin(a1) * ry);
    }
    g.stroke({ width: 1.6, color: COL_THREAT, alpha: 0.4 + 0.35 * pulse });
    const caption = siege.badge ?? "LANDING UNDER WAY";
    if (siege.progress !== null) {
      const r2 = rx + 7;
      const end = -Math.PI / 2 + siege.progress * Math.PI * 2;
      const steps = Math.max(2, Math.ceil(siege.progress * 48));
      for (let i = 0; i <= steps; i++) {
        const a = -Math.PI / 2 + (end + Math.PI / 2) * (i / steps);
        const x = body.sx + Math.cos(a) * r2;
        const y = body.sy + Math.sin(a) * r2 * RING_TILT;
        if (i === 0) g.moveTo(x, y);
        else g.lineTo(x, y);
      }
      g.stroke({ width: 2.4, color: COL_SIEGE, alpha: 0.9 });
    }
    // The siege is the star's headline: its status reads in the header band
    // (always legible); the cordon, clock and guns stay drawn at the world.
    const hx = frame.headline.x;
    const hy = frame.headline.y + 12;
    this.text(`${caption.startsWith("CONTEST") ? "⚔" : "⛔"} ${caption}`, hx, hy, COL_THREAT, 0.5, 10, true);
    const detail: string[] = [];
    if (siege.landing) detail.push(`LANDING ${Math.round(siege.landing.marines * 100)}% vs ${Math.round(siege.landing.defenders * 100)}%`);
    if (siege.suppression > 0.02) detail.push(`GARRISON PINNED ${Math.round(siege.suppression * 100)}%`);
    if (detail.length) this.text(detail.join(" · "), hx, hy + 14, COL_SIEGE, 0.5, 9, true);

    // Bombardment: the cordon's guns on the garrison while suppression holds.
    const besiegers = picture.fleets.filter((fleet) => fleet.placement.mode === "cordon" && fleet.placement.body === siege.target);
    if (siege.suppression > 0.02) {
      const slot = Math.floor(now / 260);
      const rnd = seeded(hash(`${picture.systemId}:bombard`) ^ slot);
      const phase = (now % 260) / 260;
      const sources = besiegers.map((fleet) => this.markers.get(fleet.id)).filter((m): m is FleetMarker => !!m?.present);
      const beams = 1 + Math.round(siege.suppression * 3);
      for (let i = 0; i < beams; i++) {
        const src = sources.length
          ? sources[Math.floor(rnd() * sources.length) % sources.length]
          : null;
        const a = rnd() * Math.PI * 2;
        const sxp = src ? src.x : body.sx + Math.cos(a) * rx;
        const syp = src ? src.y : body.sy + Math.sin(a) * ry;
        const hitA = Math.atan2(syp - body.sy, sxp - body.sx) + (rnd() - 0.5) * 0.9;
        const hx = body.sx + Math.cos(hitA) * body.r * 0.82;
        const hy = body.sy + Math.sin(hitA) * body.r * 0.82;
        this.over.moveTo(sxp, syp).lineTo(sxp + (hx - sxp) * Math.min(1, phase * 2), syp + (hy - syp) * Math.min(1, phase * 2))
          .stroke({ width: 1.3, color: 0xffb47a, alpha: 0.85 });
        if (phase > 0.5) this.over.circle(hx, hy, 1.5 + (phase - 0.5) * 6).fill({ color: 0xffe3a8, alpha: (1 - phase) * 1.6 });
      }
    }

    // A landing in progress: troop craft descending from the cordon.
    if (siege.landing) {
      const t = (now % 2400) / 2400;
      for (let i = 0; i < 3; i++) {
        const k = (t + i / 3) % 1;
        const a = -Math.PI / 2 + (i - 1) * 0.55;
        const x0 = body.sx + Math.cos(a) * rx;
        const y0 = body.sy + Math.sin(a) * ry;
        const x = x0 + (body.sx - x0) * k * 0.8;
        const y = y0 + (body.sy - y0) * k * 0.8;
        const dir = Math.atan2(body.sy - y0, body.sx - x0);
        const c = Math.cos(dir), s = Math.sin(dir);
        this.over.poly([x + c * 4, y + s * 4, x - c * 3 - s * 3, y - s * 3 + c * 3, x - c * 3 + s * 3, y - s * 3 - c * 3])
          .fill({ color: 0xffe08a, alpha: 0.9 * (1 - k * 0.6) });
      }
      this.sites.push({ kind: "landing", id: siege.landing.id, x: hx, y: hy + 8, r: 46,
        hover: "Landing in progress · follow the ground fight as its light arrives" });
    }
  }

  private bracket(x: number, y: number, half: number, color: number): void {
    const arm = 7;
    for (const dx of [-1, 1]) for (const dy of [-1, 1]) {
      this.over.moveTo(x + dx * (half - arm), y + dy * half).lineTo(x + dx * half, y + dy * half)
        .lineTo(x + dx * half, y + dy * (half - arm));
    }
    this.over.stroke({ width: 1.6, color, alpha: 0.95 });
  }
}

/** What a contact is called on the System View (hull culture, not class, for
 * pirates; the Authority's own services by name). */
function fleetName(g: GhostView): string {
  if (g.tca) return g.migrant ? "Migrant Liner" : g.rescue_service ? "AAA Rescue Tender" : "Authority Freighter";
  if (g.pirate) return g.kind === "raider" ? "Pirate raiders" : `Pirate ${KIND_LABEL[g.kind]}`;
  return KIND_LABEL[g.kind];
}

function fleetHover(fleet: PictureFleet): string {
  const g = fleet.ghost;
  const kind = fleetName(g);
  const p = fleet.placement;
  const where = p.mode === "berth" ? "berthed here"
    : p.mode === "belt" ? (fleet.ghost.anchor ? "anchored at the belt" : "mining the belt")
    : p.mode === "cordon" ? "holding orbit over this world"
      : p.mode === "battle" ? "engaged"
        : p.mode === "inbound" ? `inbound${p.eta !== null ? ` · ETA ${formatAge(p.eta)}` : ""}`
          : p.mode === "outbound" ? (p.to ? `heading to ${p.to}${p.eta !== null ? ` · ETA ${formatAge(p.eta)}` : ""}` : "leaving")
            : "near the star";
  const owner = fleet.tone === "ally" ? "Ally " : fleet.tone === "rival" ? "Rival " : "";
  const age = g.age >= 1 ? ` · seen Δ${formatAge(g.age)} ago` : "";
  return `${owner}${kind} · ${where}${age}`;
}

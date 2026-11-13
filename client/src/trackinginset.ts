// ============================================================================
// TRACKING INSET — a zoomed camera on the fleet selected in the System View
// (Peter, Oct 27 2026: "keep the ships the current size but when you click
// the ship, show a small 'tracking zoomed in view'"), and on the raid coming
// in (the raid camera, render.ts).
// ============================================================================
//
// The inset draws a System View scene of its own (render.ts keeps it in step
// with the orrery's data) with its own camera: a fixed scale, locked on the
// fleet. Panning or zooming the orrery never reframes it (Peter: "the tracking
// window shouldn't move its frame when the player pans or zooms"). The scene
// lays out in a viewport the inset's size divided by the magnification, and
// the texture's resolution gives the pixels back, so hulls, worlds and the
// live 3D hull all magnify sharply together. A selected fleet's inset sits in
// the map area's top-left corner (the toast lane owns the top right); the raid
// camera's sits centred just above the map's middle, with the galaxy framing
// the raid beneath it, and carries a note along its foot (the raid's news). A
// faint starfield drifts behind the scene with the fleet. The inset swallows
// its own clicks. Presentation only.

import {
  Assets, Container, Graphics, Matrix, RenderTexture, Sprite, Text, TextStyle, Texture, TilingSprite, type Renderer,
} from "pixi.js";
import { hashId, mulberry32 } from "./prng";

/** §tracking (Peter, Oct 28 2026: "shouldn't the mining barge tracking frame
 * show it mining ore"): a tracked barge cutting ore works among its belt's
 * rocks — their art URL — and a cutting beam. The orrery scatters too few rocks
 * along a whole belt for any to fall inside the inset's close frame. */
export interface TrackingWork {
  rockUrl: string;
}
/** Rocks around a working barge, and the beam's colour. */
const WORK_ROCKS = 7;
const BEAM = 0x7fe3ff;

export interface TrackingTarget {
  /** Where and how long (px) the fleet is drawn in its scene, and its place
   * in scene units from the star. */
  x: number;
  y: number;
  px: number;
  lx: number;
  ly: number;
  /** The caption's name and the fleet's tone. */
  title: string;
  color: number;
}

/** §charter-advisor (Peter, Nov 9 2026: "During the pirate raid warning, can
 * we add the advisor's portrait much larger integrated into the Pirate
 * Warning with some text"): the Charter Advisor in a column of the raid's
 * window, their face over their name and their line. */
export interface TrackingVoice {
  portrait: string;
  name: string;
  line: string;
}

/** The scene the inset draws: the tracking System View scene's camera. */
export interface TrackingCamera {
  trackCamera(viewW: number, viewH: number, scale: number, focus: { x: number; y: number }, cut: boolean): void;
}

/** Where the inset sits: a selected fleet's in the corner, the raid's centred. */
export type TrackingPlace = "corner" | "center";

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** The inset at its largest (px; Peter: "50% larger") and its widest share of
 * the map, its height to width, and its smallest map. */
const INSET_W = 450;
const INSET_SHARE = 0.51;
const INSET_ASPECT = 0.62;
const MIN_AREA_W = 420;
const MIN_AREA_H = 300;
const MARGIN = 12;
const CAPTION_H = 20;
/** The band a note sits in along the inset's foot (px). */
const NOTE_H = 22;
/** A centred inset's gap above the map's middle (the raid framed beneath). */
const CENTER_GAP = 36;
/** The tracked hull's length as a share of the inset's width, and the
 * magnification's bounds (a big hull is never shown smaller than 2.25×). */
const HULL_SHARE = 0.38;
const MAG_MIN = 2.25;
const MAG_MAX = 6;
/** The tracking scene's own scale (px a unit, before magnification): the
 * orrery's proportions at rest on a laptop, whatever its camera does now. */
export const TRACKING_SCALE = 330;
/** The map's own background (render.ts `background`). */
const BACKDROP = 0x05070d;
/** How far the starfield drifts for the fleet's move (a share of the scene's). */
const STAR_DRIFT = 0.25;
/** §charter-advisor: the advisor's column on the raid window's left (px), its
 * margin, and the Authority's gold for the face's frame and the name. */
const VOICE_W = 176;
const VOICE_PAD = 10;
const VOICE_GOLD = 0xd4b46a;
const VOICE_NAME = 0xe2c886;
const VOICE_TEXT = 0xdce0e8;

/** A tile of faint stars, seeded, so every session draws the same sky. */
function starTile(): Texture | null {
  if (typeof document === "undefined") return null;
  const size = 256;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  const rand = mulberry32(0x7a11c0de);
  for (let i = 0; i < 80; i++) {
    const r = rand() < 0.85 ? 0.45 + rand() * 0.45 : 0.9 + rand() * 0.6;
    ctx.globalAlpha = 0.2 + rand() * 0.55;
    const tint = rand();
    ctx.fillStyle = tint < 0.7 ? "#e6eeff" : tint < 0.85 ? "#9fc4ff" : "#ffdcb0";
    ctx.beginPath();
    ctx.arc(3 + rand() * (size - 6), 3 + rand() * (size - 6), r, 0, Math.PI * 2);
    ctx.fill();
  }
  return Texture.from(canvas);
}

export class TrackingInset {
  readonly root = new Container();
  private readonly pane = new Container();
  private readonly backdrop = new Graphics();
  private stars: TilingSprite | null = null;
  private readonly view = new Sprite();
  private readonly band = new Graphics();
  private readonly mask = new Graphics();
  private readonly frame = new Graphics();
  private readonly caption = new Text({
    text: "",
    style: new TextStyle({ fontFamily: "ui-monospace, monospace", fontSize: 10, fontWeight: "700", letterSpacing: 1,
      fill: 0xffffff, dropShadow: { color: 0x02040a, alpha: 0.9, blur: 2, distance: 1 } }),
  });
  private readonly note = new Text({
    text: "",
    style: new TextStyle({ fontFamily: "ui-monospace, monospace", fontSize: 11, fontWeight: "700", letterSpacing: 1,
      fill: 0xffffff, dropShadow: { color: 0x02040a, alpha: 0.9, blur: 2, distance: 1 } }),
  });
  private texture: RenderTexture | null = null;
  private box: Rect | null = null;
  /// The scene's viewport (the inset over the magnification) this frame.
  private viewport = { w: 0, h: 0 };
  private mag = 1;
  private id: string | null = null;
  /// Where the fleet stood last frame (scene units): the camera's next focus.
  private focus = { x: 0, y: 0 };
  private readonly matrix = new Matrix();
  /// §tracking: the working barge's rocks (behind the scene) and its beam and
  /// ore (over it); `rockTex` per art URL, loaded once.
  private readonly rocks = new Container();
  private readonly beam = new Graphics();
  private readonly rockTex = new Map<string, Texture | null>();
  /// §charter-advisor: who speaks beside the raid's window, if anyone, and
  /// their column: backdrop and face frame, face, name and line.
  private voice: TrackingVoice | null = null;
  private readonly voicePanel = new Graphics();
  private readonly voiceFace = new Sprite();
  private readonly voiceName = new Text({
    text: "",
    style: new TextStyle({ fontFamily: "ui-monospace, monospace", fontSize: 10, fontWeight: "700", letterSpacing: 1,
      fill: VOICE_NAME, wordWrap: true, wordWrapWidth: VOICE_W - 2 * VOICE_PAD }),
  });
  private readonly voiceLine = new Text({
    text: "",
    style: new TextStyle({ fontFamily: "ui-monospace, monospace", fontSize: 12, lineHeight: 16,
      fill: VOICE_TEXT, wordWrap: true, wordWrapWidth: VOICE_W - 2 * VOICE_PAD }),
  });
  private readonly faceTex = new Map<string, Texture | null>();

  constructor() {
    this.root.eventMode = "none";
    this.root.visible = false;
    this.pane.addChild(this.backdrop, this.rocks, this.view, this.beam, this.band);
    this.pane.mask = this.mask;
    this.note.visible = false;
    this.voicePanel.visible = this.voiceFace.visible = this.voiceName.visible = this.voiceLine.visible = false;
    this.root.addChild(this.pane, this.mask, this.voicePanel, this.voiceFace, this.voiceName, this.voiceLine,
      this.frame, this.caption, this.note);
  }

  /** Whether a screen point lies on the inset (and its advisor's column)
   * while it is shown. */
  contains(x: number, y: number): boolean {
    const b = this.root.visible ? this.box : null;
    const left = b && this.voice ? b.x - VOICE_W : b?.x ?? 0;
    return !!b && x >= left && x <= b.x + b.w && y >= b.y && y <= b.y + b.h;
  }

  /** §charter-advisor: the advisor is on screen beside the raid's window. */
  voiced(): boolean {
    return this.root.visible && !!this.voice;
  }

  /** The inset's centre on screen (where its live 3D hull counts as drawn). */
  center(): { x: number; y: number } {
    const b = this.box;
    return b ? { x: b.x + b.w / 2, y: b.y + b.h / 2 } : { x: 0, y: 0 };
  }

  hide(): void {
    this.root.visible = false;
    this.id = null;
    this.voice = null;
  }

  /** Before the tracking scene updates: frame its camera on the fleet. `seen`
   * is the fleet as last drawn (its hull length sets the magnification; a new
   * fleet starts from its place there). False when the map is too small to
   * spare the inset. A centred (raid) window can carry `voice` in a column on
   * its left, the two centred together. */
  prepare(scene: TrackingCamera, id: string, seen: TrackingTarget, area: Rect, where: TrackingPlace = "corner",
    voice: TrackingVoice | null = null): boolean {
    this.voice = where === "center" ? voice : null;
    const box = this.place(area, where, !!this.voice);
    if (!box) {
      this.hide();
      return false;
    }
    // A new fleet, or the inset shown again, is a new shot.
    const cut = id !== this.id || !this.root.visible;
    if (cut) this.focus = { x: seen.lx, y: seen.ly };
    this.id = id;
    this.box = box;
    this.mag = Math.max(MAG_MIN, Math.min(MAG_MAX, (HULL_SHARE * box.w) / Math.max(1, seen.px)));
    this.viewport = { w: Math.round(box.w / this.mag), h: Math.round(box.h / this.mag) };
    scene.trackCamera(this.viewport.w, this.viewport.h, TRACKING_SCALE, this.focus, cut);
    return true;
  }

  /** After the scene's hulls (and their live 3D models) are placed: draw it
   * into the inset with the fleet exactly at the centre, beneath the caption,
   * over the starfield, and `note` (if any) along its foot, shrunk to fit. */
  render(renderer: Renderer, scene: Container, target: TrackingTarget, note: string | null = null,
    work: TrackingWork | null = null): void {
    const box = this.box;
    if (!box) return;
    const { w, h } = this.viewport;
    const resolution = renderer.resolution * (box.w / w);
    if (!this.texture) {
      this.texture = RenderTexture.create({ width: w, height: h, resolution, antialias: true });
      this.view.texture = this.texture;
    } else if (this.texture.width !== w || this.texture.height !== h || this.texture.source.resolution !== resolution) {
      this.texture.resize(w, h, resolution);
    }
    this.matrix.identity().translate(w / 2 - target.x, (h + CAPTION_H * h / box.h) / 2 - target.y);
    renderer.render({ container: scene, target: this.texture, clear: true, clearColor: [0, 0, 0, 0], transform: this.matrix });
    this.focus = { x: target.lx, y: target.ly };

    this.backdrop.clear().rect(box.x, box.y, box.w, box.h).fill(BACKDROP);
    if (!this.stars) {
      const tile = starTile();
      if (tile) this.stars = this.pane.addChildAt(new TilingSprite({ texture: tile, width: box.w, height: box.h }), 1);
    }
    if (this.stars) {
      this.stars.position.set(box.x, box.y);
      this.stars.width = box.w;
      this.stars.height = box.h;
      this.stars.tilePosition.set(-target.lx * TRACKING_SCALE * STAR_DRIFT, -target.ly * TRACKING_SCALE * STAR_DRIFT);
    }
    this.view.position.set(box.x, box.y);
    this.view.width = box.w;
    this.view.height = box.h;
    this.drawWork(box, target, work);
    this.band.clear().rect(box.x, box.y, box.w, CAPTION_H).fill({ color: BACKDROP, alpha: 0.6 });
    this.note.visible = !!note;
    if (note) {
      this.band.rect(box.x, box.y + box.h - NOTE_H, box.w, NOTE_H).fill({ color: BACKDROP, alpha: 0.6 });
      if (this.note.text !== note) this.note.text = note;
      this.note.style.fill = target.color;
      this.note.scale.set(1);
      this.note.scale.set(Math.min(1, (box.w - 16) / Math.max(1, this.note.width)));
      this.note.position.set(box.x + (box.w - this.note.width) / 2, box.y + box.h - NOTE_H + (NOTE_H - this.note.height) / 2);
    }
    this.mask.clear().roundRect(box.x, box.y, box.w, box.h, 6).fill(0xffffff);
    this.drawVoice(box);
    // One frame round the window and the advisor's column, a rule between.
    const left = this.voice ? box.x - VOICE_W : box.x;
    this.frame.clear().roundRect(left + 0.5, box.y + 0.5, box.x + box.w - left - 1, box.h - 1, 6)
      .stroke({ width: 1, color: target.color, alpha: 0.75 });
    if (this.voice) this.frame.moveTo(box.x + 0.5, box.y + 6).lineTo(box.x + 0.5, box.y + box.h - 6)
      .stroke({ width: 1, color: target.color, alpha: 0.35 });
    const text = `◉ TRACKING · ${target.title}`;
    if (this.caption.text !== text) this.caption.text = text;
    this.caption.style.fill = target.color;
    this.caption.position.set(box.x + 8, box.y + (CAPTION_H - this.caption.height) / 2);
    this.root.visible = true;
  }

  /** §tracking: a barge at work — rocks of its belt around it, the nearest
   * under a pulsing cutting beam, ore flowing back to the hold. The fleet sits
   * at the inset's centre, `target.px × mag` long. */
  private drawWork(box: Rect, target: TrackingTarget, work: TrackingWork | null): void {
    this.beam.clear();
    const tex = work ? this.rockTexture(work.rockUrl) : null;
    if (!work || !tex) {
      this.rocks.visible = false;
      return;
    }
    this.rocks.visible = true;
    // Where the live 3D hull is drawn (`center`).
    const cx = box.x + box.w / 2;
    const cy = box.y + box.h / 2;
    const hull = Math.max(12, target.px * this.mag);
    const now = performance.now() / 1000;
    const rand = mulberry32(hashId(this.id ?? "barge"));
    while (this.rocks.children.length < WORK_ROCKS) this.rocks.addChild(new Sprite());
    let cut = { x: cx, y: cy };
    for (let i = 0; i < WORK_ROCKS; i++) {
      const rock = this.rocks.children[i] as Sprite;
      // The first rock is the one under the beam: close, ahead and below.
      const angle = i === 0 ? 0.35 : (i / WORK_ROCKS) * Math.PI * 2 + rand() * 0.6;
      const reach = hull * (i === 0 ? 0.95 : 1.05 + rand() * 0.9);
      const size = hull * (i === 0 ? 0.62 : 0.28 + rand() * 0.45);
      rock.texture = tex;
      rock.anchor.set(0.5);
      rock.width = rock.height = size;
      rock.position.set(cx + Math.cos(angle) * reach, cy + Math.sin(angle) * reach * 0.62);
      rock.rotation = rand() * Math.PI * 2 + now * (0.04 + rand() * 0.05) * (i % 2 ? 1 : -1);
      rock.alpha = i === 0 ? 1 : 0.85;
      if (i === 0) cut = { x: rock.position.x, y: rock.position.y };
    }
    // The cutting beam leaves the hull's edge and pulses; ore motes ride it
    // back to the hold.
    const span = Math.hypot(cut.x - cx, cut.y - cy) || 1;
    const sx = cx + (cut.x - cx) / span * hull * 0.3;
    const sy = cy + (cut.y - cy) / span * hull * 0.3;
    const pulse = 0.55 + 0.45 * Math.sin(now * 7);
    this.beam.moveTo(sx, sy).lineTo(cut.x, cut.y).stroke({ width: 6, color: BEAM, alpha: 0.2 * pulse });
    this.beam.moveTo(sx, sy).lineTo(cut.x, cut.y).stroke({ width: 1.8, color: BEAM, alpha: 0.75 * pulse + 0.2 });
    this.beam.circle(cut.x, cut.y, 3 + 2.5 * pulse).fill({ color: 0xffe2a8, alpha: 0.5 + 0.4 * pulse });
    for (let i = 0; i < 6; i++) {
      const t = (now * 0.6 + i / 6) % 1;
      this.beam.circle(cut.x + (sx - cut.x) * t, cut.y + (sy - cut.y) * t, 2.2).fill({ color: 0xffc46b, alpha: 0.95 * (1 - t) });
    }
  }

  /** §charter-advisor: the advisor's column on the window's left: the face
   * as large as the column allows, framed in gold, over their name and line. */
  private drawVoice(box: Rect): void {
    const v = this.voice;
    this.voicePanel.clear();
    this.voicePanel.visible = this.voiceName.visible = this.voiceLine.visible = !!v;
    if (!v) {
      this.voiceFace.visible = false;
      return;
    }
    const x = box.x - VOICE_W;
    const name = v.name.toUpperCase();
    if (this.voiceName.text !== name) this.voiceName.text = name;
    if (this.voiceLine.text !== v.line) this.voiceLine.text = v.line;
    const words = this.voiceName.height + 4 + this.voiceLine.height;
    const face = Math.round(Math.max(48, Math.min(VOICE_W - 2 * VOICE_PAD, box.h - 3 * VOICE_PAD - words)));
    const fx = x + (VOICE_W - face) / 2;
    const fy = box.y + VOICE_PAD;
    this.voicePanel.roundRect(x, box.y, VOICE_W, box.h, 6).fill({ color: BACKDROP, alpha: 0.94 })
      .roundRect(fx - 1.5, fy - 1.5, face + 3, face + 3, 6).stroke({ width: 1.5, color: VOICE_GOLD });
    const tex = this.faceTexture(v.portrait);
    this.voiceFace.visible = !!tex;
    if (tex) {
      this.voiceFace.texture = tex;
      this.voiceFace.position.set(fx, fy);
      this.voiceFace.width = this.voiceFace.height = face;
    }
    this.voiceName.position.set(x + VOICE_PAD, fy + face + VOICE_PAD);
    this.voiceLine.position.set(x + VOICE_PAD, fy + face + VOICE_PAD + this.voiceName.height + 4);
  }

  /** §charter-advisor: start loading a face before its raid, so the
   * warning never opens on an empty frame. */
  preloadFace(url: string): void {
    this.faceTexture(url);
  }

  private faceTexture(url: string): Texture | null {
    if (!this.faceTex.has(url)) {
      this.faceTex.set(url, null);
      void Assets.load<Texture>(url).then((texture) => this.faceTex.set(url, texture)).catch(() => undefined);
    }
    return this.faceTex.get(url) ?? null;
  }

  private rockTexture(url: string): Texture | null {
    if (!this.rockTex.has(url)) {
      this.rockTex.set(url, null);
      void Assets.load<Texture>(url).then((texture) => this.rockTex.set(url, texture)).catch(() => undefined);
    }
    return this.rockTex.get(url) ?? null;
  }

  /** The inset's place in a map area: its top-left corner, or centred just
   * above the middle (with an advisor's column, the two centred together);
   * nowhere when the map is too small to spare it. */
  place(area: Rect, where: TrackingPlace, voiced = false): Rect | null {
    if (area.w < MIN_AREA_W || area.h < MIN_AREA_H) return null;
    const w = Math.round(Math.min(INSET_W, area.w * INSET_SHARE));
    const h = Math.round(w * INSET_ASPECT);
    if (where === "corner") return { x: Math.round(area.x + MARGIN), y: Math.round(area.y + MARGIN), w, h };
    const y = Math.max(area.y + MARGIN, area.y + area.h / 2 - CENTER_GAP - h);
    const side = voiced ? VOICE_W : 0;
    return { x: Math.round(area.x + (area.w - w - side) / 2 + side), y: Math.round(y), w, h };
  }
}

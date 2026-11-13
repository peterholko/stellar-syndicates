// ============================================================================
// STAR IDENTITY — per-type motion for the System View's star. Cosmetic only.
// ============================================================================
//
// Each type keeps its approved artwork and measured visible size (the galaxy
// handoff depends on that size). Identity comes from MOTION and light layered
// around it: a surface turning about its north–south axis (starroll.ts: a
// binary's two stars each on their own, a pulsar's beams swinging round like a
// lighthouse's), a magnetar's flicker and flares, a red giant's swollen
// envelope, a black hole's infall. Everything is a pure function of wall time,
// so pausing or re-entering a system never accumulates state.

import { Container, Graphics, Sprite } from "pixi.js";
import { glowSpriteTexture } from "./systembodyfx";
import { ORBIT_TILT } from "./systemlayout";
import { starPeriod, starRoll, type RollingStar } from "./starroll";

interface StarMotion {
  /** Corona size relative to the visible disc, and its breathing depth. */
  corona: number;
  breathe: number;
  /** Extra outer envelope (red giants). */
  envelope: number;
  flicker: number;
}

const MOTION: Record<string, StarMotion> = {
  red_dwarf: { corona: 1.9, breathe: 0.08, envelope: 0, flicker: 0.04 },
  yellow_star: { corona: 2.1, breathe: 0.06, envelope: 0, flicker: 0 },
  white_star: { corona: 2.1, breathe: 0.05, envelope: 0, flicker: 0 },
  blue_giant: { corona: 2.4, breathe: 0.07, envelope: 0, flicker: 0 },
  red_giant: { corona: 2.0, breathe: 0.1, envelope: 3.2, flicker: 0 },
  white_dwarf: { corona: 2.6, breathe: 0.04, envelope: 0, flicker: 0 },
  neutron_star: { corona: 1.6, breathe: 0, envelope: 0, flicker: 0.35 },
  binary_star: { corona: 1.7, breathe: 0.05, envelope: 0, flicker: 0 },
  black_hole: { corona: 1.5, breathe: 0.12, envelope: 0, flicker: 0 },
  magnetar: { corona: 1.9, breathe: 0.1, envelope: 0, flicker: 0.25 },
};

/** Shapes above the star are drawn this many times larger, then scaled back.
 * Pixi picks a circle's segment count from its LOCAL radius, and the star's
 * normalized units (a disc ~0.085 across) would otherwise turn every flare
 * into an octagon once the scene magnifies it a few hundredfold. */
const DETAIL = 1000;
/** The pulsar's flash, as a soft glow this many star radii across. */
const FLASH_DIAMETER = 2.4;

const TINT: Record<string, number> = {
  red_dwarf: 0xff8a5c, yellow_star: 0xffd27a, white_star: 0xdfe8ff, blue_giant: 0x9fc4ff,
  red_giant: 0xff6a3c, white_dwarf: 0xd8e6ff, neutron_star: 0x8fc8ff, binary_star: 0xffcf8a,
  black_hole: 0xff9a4a, magnetar: 0xb98cff,
};

export class StarFx {
  /** Behind the star sprite (corona/envelope), in normalized star units. */
  readonly under = new Container();
  /** Above the star sprite (flares, infall motes, the pulsar's flash). */
  readonly over = new Container();
  private readonly shapes = new Graphics();
  private corona: Sprite | null = null;
  private envelope: Sprite | null = null;
  private flash: Sprite | null = null;
  private slug = "";
  private radius = 0.085; // visible star radius, normalized

  constructor() {
    this.under.eventMode = "none";
    this.over.eventMode = "none";
    this.shapes.scale.set(1 / DETAIL);
    this.over.addChild(this.shapes);
  }

  /** (Re)dress for a star type at a visible radius in normalized units. */
  configure(slug: string, radius: number): void {
    this.slug = slug;
    this.radius = radius;
    const texture = glowSpriteTexture();
    if (!texture) return;
    const motion = MOTION[slug] ?? MOTION.yellow_star;
    const tint = TINT[slug] ?? 0xffe08a;
    // Created once, envelope first so it stays beneath the corona.
    this.envelope ??= this.under.addChild(new Sprite(texture));
    this.corona ??= this.under.addChild(new Sprite(texture));
    this.corona.anchor.set(0.5);
    this.corona.tint = tint;
    this.corona.scale.set((radius * 2 * motion.corona) / texture.width);
    this.corona.alpha = 0.32;
    this.envelope.anchor.set(0.5);
    this.envelope.tint = tint;
    this.envelope.visible = motion.envelope > 0;
    if (motion.envelope > 0) {
      this.envelope.scale.set((radius * 2 * motion.envelope) / texture.width);
      this.envelope.alpha = 0.16;
    }
    // The lighthouse flash is light, not a disc: a soft additive bloom.
    this.flash ??= this.over.addChild(new Sprite(texture));
    this.flash.anchor.set(0.5);
    this.flash.tint = 0xe6f4ff;
    this.flash.blendMode = "add";
    this.flash.scale.set((radius * FLASH_DIAMETER) / texture.width);
    this.flash.alpha = 0;
    this.flash.visible = slug === "neutron_star";
  }

  /** Animate at wall time `t` (s). `star` is the star artwork (null while
   * art loads); its scale is never touched, only its surface's turn. */
  update(t: number, star: RollingStar | null): void {
    const motion = MOTION[this.slug] ?? MOTION.yellow_star;
    star?.roll(t);
    const breath = 0.5 + 0.5 * Math.sin(t * 0.9);
    const flicker = motion.flicker > 0
      ? motion.flicker * (0.5 + 0.5 * Math.sin(t * (Math.PI * 2) / Math.max(0.4, starPeriod(this.slug) / 2)))
      : 0;
    if (this.corona) {
      const base = this.corona.texture.width;
      const k = 1 + motion.breathe * breath;
      this.corona.scale.set((this.radius * 2 * motion.corona * k) / base);
      this.corona.alpha = 0.26 + 0.1 * breath + flicker * 0.5;
    }
    if (this.envelope?.visible) {
      const base = this.envelope.texture.width;
      this.envelope.scale.set((this.radius * 2 * motion.envelope * (1 + 0.05 * Math.sin(t * 0.35))) / base);
      this.envelope.alpha = 0.13 + 0.05 * breath;
    }
    const g = this.shapes;
    g.clear();
    const r = this.radius * DETAIL;
    if (this.slug === "magnetar") {
      // A flare shell every few seconds, expanding and fading.
      const phase = (t / 6.5) % 1;
      g.circle(0, 0, r * (1.1 + phase * 2.4)).stroke({ width: r * 0.06, color: 0xd6b8ff, alpha: (1 - phase) * 0.45 });
    } else if (this.slug === "black_hole") {
      // Infalling motes spiral along the tilted disc into the shadow.
      for (let i = 0; i < 14; i++) {
        const life = ((t * 0.12 + i / 14) % 1);
        const a = i * 2.4 + life * Math.PI * 3.2;
        const rr = r * (2.3 - life * 1.55);
        g.circle(Math.cos(a) * rr, Math.sin(a) * rr * ORBIT_TILT * 0.55, r * 0.035 * (1.4 - life))
          .fill({ color: 0xffc27a, alpha: 0.75 * (1 - life * 0.7) });
      }
    } else if (this.slug === "neutron_star" && this.flash) {
      // The lighthouse pulse: a bright flash each time a beam swings round
      // to face the viewer (a quarter turn from its painted pose).
      const pulse = Math.pow(Math.abs(Math.sin(starRoll(this.slug, t))), 24);
      this.flash.alpha = 0.6 * pulse;
    }
  }
}

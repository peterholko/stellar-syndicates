// Soft particle textures, painted once at startup (no art files), and a
// per-frame sprite pool for the battle viewer's effects. Light (beams, flames,
// sparks, flashes) is drawn additively so overlapping effects brighten the way
// light does; smoke uses normal blending. Sprites are reused every frame —
// nothing is allocated per frame once a pool has grown to the scene's need.
import { Rectangle, Sprite, Texture, type Container } from "pixi.js";
import type { DebrisCut } from "./battlewrecks";

export interface FxTextures {
  /** Soft round glow. */ soft: Texture;
  /** Dense round blob with a soft rim (fire lobes, hot cores). */ hot: Texture;
  /** Cross-section of a beam: soft across, uniform along. */ beam: Texture;
  /** Streak bright at its head (right edge), fading to its tail. */ spark: Texture;
  /** Soft annulus (shock fronts). */ ring: Texture;
  /** Irregular smoke puff. */ puff: Texture;
  /** Four-point lens flare (muzzle flashes, glints). */ flare: Texture;
  /** Engine flame: bright at its top edge (the nozzle), tapering down. */ plume: Texture;
  /** A torn hull plate (debris). */ plate: Texture;
  /** A broken strut or beam (debris). */ strut: Texture;
}

function paint(w: number, h: number, draw: (ctx: CanvasRenderingContext2D) => void): Texture | null {
  if (typeof document === "undefined") return null;
  try {
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    draw(ctx);
    const texture = Texture.from(canvas);
    texture.source.scaleMode = "linear";
    return texture;
  } catch {
    return null;
  }
}

type Stops = [number, number][];
const white = (a: number) => `rgba(255,255,255,${a})`;
function radial(ctx: CanvasRenderingContext2D, cx: number, cy: number, r: number, stops: Stops): void {
  const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
  for (const [at, a] of stops) g.addColorStop(at, white(a));
  ctx.fillStyle = g;
  ctx.beginPath();
  ctx.arc(cx, cy, r, 0, Math.PI * 2);
  ctx.fill();
}
function linear(ctx: CanvasRenderingContext2D, x0: number, y0: number, x1: number, y1: number, stops: Stops): CanvasGradient {
  const g = ctx.createLinearGradient(x0, y0, x1, y1);
  for (const [at, a] of stops) g.addColorStop(at, white(a));
  return g;
}

/** Paint every effect texture, or null where no canvas exists (tests). */
export function createFxTextures(): FxTextures | null {
  const soft = paint(64, 64, (c) => radial(c, 32, 32, 32, [[0, 1], [0.18, 0.75], [0.42, 0.3], [0.7, 0.07], [1, 0]]));
  const hot = paint(64, 64, (c) => radial(c, 32, 32, 32, [[0, 1], [0.5, 0.9], [0.72, 0.45], [0.9, 0.1], [1, 0]]));
  const beam = paint(4, 64, (c) => {
    c.fillStyle = linear(c, 0, 0, 0, 64, [[0, 0], [0.2, 0.04], [0.36, 0.3], [0.5, 1], [0.64, 0.3], [0.8, 0.04], [1, 0]]);
    c.fillRect(0, 0, 4, 64);
  });
  const spark = paint(64, 16, (c) => {
    c.fillStyle = linear(c, 0, 0, 64, 0, [[0, 0], [0.55, 0.3], [0.9, 1], [1, 0.7]]);
    c.fillRect(0, 0, 64, 16);
    c.globalCompositeOperation = "destination-in";
    c.fillStyle = linear(c, 0, 0, 0, 16, [[0, 0], [0.5, 1], [1, 0]]);
    c.fillRect(0, 0, 64, 16);
  });
  const ring = paint(128, 128, (c) => radial(c, 64, 64, 64, [[0, 0], [0.62, 0], [0.78, 0.55], [0.86, 1], [0.92, 0.5], [1, 0]]));
  const puff = paint(64, 64, (c) => {
    radial(c, 32, 32, 30, [[0, 0.7], [0.55, 0.35], [1, 0]]);
    // Fixed lobes give the puff an irregular edge; rotation varies it per use.
    for (const [dx, dy, r] of [[-9, -6, 15], [10, -8, 13], [8, 9, 14], [-8, 10, 12], [0, -13, 11]]) {
      radial(c, 32 + dx, 32 + dy, r, [[0, 0.32], [1, 0]]);
    }
  });
  const flare = paint(64, 64, (c) => {
    for (const [sx, sy] of [[1, 0.09], [0.09, 1]]) {
      c.save();
      c.translate(32, 32);
      c.scale(sx, sy);
      radial(c, 0, 0, 32, [[0, 1], [0.3, 0.45], [1, 0]]);
      c.restore();
    }
    radial(c, 32, 32, 11, [[0, 1], [1, 0]]);
  });
  const plume = paint(32, 128, (c) => {
    for (let y = 0; y < 128; y++) {
      const t = y / 127;
      const half = 15 * Math.pow(1 - t, 0.55);
      if (half < 0.3) continue;
      c.fillStyle = linear(c, 16 - half, 0, 16 + half, 0, [[0, 0], [0.5, Math.pow(1 - t, 1.6)], [1, 0]]);
      c.fillRect(16 - half, y, half * 2, 1);
      if (t < 0.35) {
        const core = 6 * (1 - t / 0.35) + 0.5;
        c.fillStyle = linear(c, 16 - core, 0, 16 + core, 0, [[0, 0], [0.5, 0.8 * (1 - t / 0.35)], [1, 0]]);
        c.fillRect(16 - core, y, core * 2, 1);
      }
    }
  });
  // Debris: an irregular torn plate with a lit edge, and a broken strut.
  const plate = paint(32, 32, (c) => {
    const pts: [number, number][] = [[4, 9], [15, 3], [27, 7], [29, 20], [19, 29], [6, 25]];
    const g = c.createLinearGradient(4, 3, 29, 29);
    g.addColorStop(0, white(1)); g.addColorStop(1, white(0.55));
    c.fillStyle = g;
    c.beginPath(); pts.forEach(([x, y], i) => (i ? c.lineTo(x, y) : c.moveTo(x, y))); c.closePath(); c.fill();
    c.strokeStyle = white(1); c.lineWidth = 1.2; c.beginPath(); c.moveTo(4, 9); c.lineTo(15, 3); c.lineTo(27, 7); c.stroke();
  });
  const strut = paint(32, 8, (c) => {
    c.fillStyle = linear(c, 0, 0, 0, 8, [[0, 0.2], [0.35, 1], [0.7, 0.75], [1, 0.15]]);
    c.beginPath(); c.moveTo(1, 2); c.lineTo(31, 1); c.lineTo(29, 7); c.lineTo(3, 6); c.closePath(); c.fill();
  });
  if (!soft || !hot || !beam || !spark || !ring || !puff || !flare || !plume || !plate || !strut) return null;
  return { soft, hot, beam, spark, ring, puff, flare, plume, plate, strut };
}

/// A hull's debris, cut from its own art: torn plates and long spars.
export interface DebrisAtlas { plates: Texture[]; slivers: Texture[] }

/// The opacity of an image region at (u, v) (fractions of its width and
/// height), read once from a small copy; null where no canvas can read it.
export function artOpacity(image: CanvasImageSource, sx: number, sy: number, sw: number, sh: number,
  grid = 48): ((u: number, v: number) => number) | null {
  if (typeof document === "undefined" || !(sw > 0) || !(sh > 0)) return null;
  try {
    const gw = grid, gh = Math.max(4, Math.min(256, Math.round((grid * sh) / sw)));
    const canvas = document.createElement("canvas");
    canvas.width = gw;
    canvas.height = gh;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(image, sx, sy, sw, sh, 0, 0, gw, gh);
    const alpha = ctx.getImageData(0, 0, gw, gh).data;
    return (u, v) => {
      const x = Math.floor(u * gw), y = Math.floor(v * gh);
      return x < 0 || y < 0 || x >= gw || y >= gh ? 0 : alpha[(y * gw + x) * 4 + 3] / 255;
    };
  } catch {
    return null; // an unreadable (cross-origin) image keeps the painted plates
  }
}

/// Cut a hull's debris out of its own art, once: each piece is the art under
/// its torn outline, charred toward the rim, with one torn edge catching the
/// light. Char and highlights land only on metal (the art's silhouette stays).
export function paintDebrisAtlas(image: CanvasImageSource, sx: number, sy: number, sw: number, sh: number,
  cuts: DebrisCut[], cell = 64): DebrisAtlas | null {
  const pad = 8, pitch = cell + pad * 2, cols = 4, half = cell / 2; // pad: mip levels stay inside a cell
  const at = (i: number): [number, number] => [(i % cols) * pitch + pad + half, Math.floor(i / cols) * pitch + pad + half];
  const atlas = paint(cols * pitch, Math.ceil(cuts.length / cols) * pitch, (c) => {
    cuts.forEach((cut, i) => {
      const [ox, oy] = at(i);
      const outline = new Path2D();
      cut.outline.forEach(([x, y], j) => (j ? outline.lineTo(ox + x * half, oy + y * half) : outline.moveTo(ox + x * half, oy + y * half)));
      outline.closePath();
      c.save();
      c.clip(outline);
      // The art, scaled so `span` of its width fills the cell and turned by
      // `rot`, with the cut's centre on the cell's centre.
      const k = cell / (cut.span * sw);
      c.translate(ox, oy);
      c.rotate(-cut.rot);
      c.scale(k, k);
      c.drawImage(image, sx, sy, sw, sh, -cut.u * sw, -cut.v * sh, sw, sh);
      c.setTransform(1, 0, 0, 1, 0, 0);
      c.globalCompositeOperation = "source-atop";
      const soot = c.createRadialGradient(ox, oy, half * 0.15, ox, oy, half * 1.05);
      soot.addColorStop(0, "rgba(20,12,8,0)");
      soot.addColorStop(0.6, "rgba(20,12,8,0.16)");
      soot.addColorStop(1, "rgba(20,12,8,0.85)");
      c.fillStyle = soot;
      c.fillRect(ox - half, oy - half, cell, cell);
      c.lineWidth = 3;
      c.strokeStyle = "rgba(14,8,5,0.55)";
      c.stroke(outline); // a scorched rim (the clip keeps its inner half)
      const lit = new Path2D();
      cut.outline.slice(0, 3).forEach(([x, y], j) => (j ? lit.lineTo(ox + x * half, oy + y * half) : lit.moveTo(ox + x * half, oy + y * half)));
      c.lineWidth = 1.6;
      c.strokeStyle = "rgba(255,238,214,0.45)";
      c.stroke(lit); // one torn edge catches the light
      c.restore();
    });
  });
  if (!atlas) return null;
  atlas.source.autoGenerateMipmaps = true; // pieces are drawn far smaller than a cell
  const frames = cuts.map((_, i) => {
    const [ox, oy] = at(i);
    return new Texture({ source: atlas.source, frame: new Rectangle(ox - half, oy - half, cell, cell) });
  });
  return {
    plates: frames.filter((_, i) => !cuts[i].sliver),
    slivers: frames.filter((_, i) => cuts[i].sliver),
  };
}

/// Sprites placed afresh each frame: begin(), draw()…, end() hides the rest.
export class FxSpritePool {
  private sprites: Sprite[] = [];
  private used = 0;

  constructor(private readonly layer: Container, private readonly blend: "add" | "normal", private readonly cap = 4000) {}

  begin(): void {
    this.used = 0;
  }

  /** One sprite of `w`×`h` canvas pixels, anchored at (x, y) by (ax, ay). */
  draw(texture: Texture, x: number, y: number, w: number, h: number, rotation: number,
    tint: number, alpha: number, ax = 0.5, ay = 0.5): void {
    if (!(alpha > 0.004) || !(w > 0.05) || !(h > 0.05) || this.used >= this.cap) return;
    let s = this.sprites[this.used];
    if (!s) {
      s = new Sprite(texture);
      s.blendMode = this.blend;
      s.eventMode = "none";
      this.layer.addChild(s);
      this.sprites.push(s);
    }
    this.used++;
    s.texture = texture;
    s.anchor.set(ax, ay);
    s.position.set(x, y);
    s.scale.set(w / texture.width, h / texture.height);
    s.rotation = rotation;
    s.tint = tint;
    s.alpha = Math.min(1, alpha);
    s.visible = true;
  }

  end(): void {
    for (let i = this.used; i < this.sprites.length; i++) this.sprites[i].visible = false;
  }

  get count(): number {
    return this.used;
  }
}

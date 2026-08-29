// §ground G3 — THE GROUND THEATER: a landing, replayed.
//
// Deliberately NOT the battle theater with different sprites. A fleet action is
// dozens of individuals manoeuvring, which is why that one is a Pixi scene of
// moving hulls. A landing is TWO MASSES and a sky: how many are left on each
// side, how many of the defenders are pinned in cover, and the moment it turned.
// Drawn on a plain 2D canvas, because what makes this fight legible is clarity
// about quantities over time, not parallax.
//
// Everything here is a PURE FUNCTION of an already-delivered, already-fog-
// filtered `GroundRecordView`. It invents nothing and reveals nothing: at bucket
// fidelity the exact counts are simply absent from the record, so the same code
// path draws the same fight with the numbers left off. There is no branch here
// that could leak — the data never arrived.

import type { GroundNote, GroundRecordView, GroundRoundView } from "./protocol";

const DEFAULT_W = 900;
const DEFAULT_H = 380;

let W = DEFAULT_W;
let H = DEFAULT_H;

export interface GroundTheaterViewport {
  width: number;
  height: number;
}

let canvas: HTMLCanvasElement | null = null;
let ctx: CanvasRenderingContext2D | null = null;
let rec: GroundRecordView | null = null;
let round = 0;
let frac = 0;
let live = false;
/// Wall-clock seconds, advanced by the host's ticker — drives idle motion only
/// (drifting smoke, falling fire). It never advances the FIGHT, which moves
/// strictly on recorded rounds.
let clock = 0;

/// A deterministic per-index jitter, so troop dots sit in a stable scatter
/// instead of boiling between frames. Same index, same offset, forever.
function scatter(i: number, salt: number): number {
  const x = Math.sin(i * 12.9898 + salt * 78.233) * 43758.5453;
  return x - Math.floor(x);
}

export function groundTheaterAvailable(): boolean {
  return typeof document !== "undefined" && !!document.createElement("canvas").getContext;
}

/// Mount the theater into `el` and bind it to a record.
export function groundTheaterAttach(el: HTMLElement, r: GroundRecordView, viewport?: GroundTheaterViewport): void {
  if (!canvas) {
    canvas = document.createElement("canvas");
    canvas.className = "gt-canvas";
    canvas.style.width = "100%";
    canvas.style.display = "block";
    ctx = canvas.getContext("2d");
  }
  const nextW = Math.max(280, Math.round(viewport?.width ?? DEFAULT_W));
  const nextH = Math.max(300, Math.round(viewport?.height ?? DEFAULT_H));
  if (W !== nextW || H !== nextH || canvas.width !== nextW || canvas.height !== nextH) {
    W = nextW;
    H = nextH;
    canvas.width = W;
    canvas.height = H;
  }
  canvas.style.height = viewport ? "100%" : "auto";
  if (canvas.parentElement !== el) {
    el.innerHTML = "";
    el.appendChild(canvas);
  }
  rec = r;
  draw();
}

/// Show `round` (+ `frac` of the way to the next). The host owns playback; this
/// module only ever renders what it is told to.
export function groundTheaterSetTime(r: number, f: number, isLive: boolean): void {
  round = r;
  frac = f;
  live = isLive;
  draw();
}

/// Advance the idle-motion clock. Does not move the fight.
export function groundTheaterStep(dt = 1 / 60): void {
  clock += dt;
  draw();
}

export function groundTheaterClose(): void {
  rec = null;
}

/// Debug/verification hook: what the theater believes it is drawing. Used by the
/// headless checks, where a canvas never paints but the derived state must
/// still be provably right.
export function groundTheaterDebug(): Record<string, unknown> | null {
  if (!rec) return null;
  const s = stateAt();
  return {
    id: rec.id,
    fidelity: rec.fidelity,
    rounds: rec.rounds.length,
    round,
    live,
    outcome: rec.outcome,
    marinesFrac: s.mFrac,
    defendersFrac: s.dFrac,
    suppression: s.supp,
    engagedFrac: s.engagedFrac,
    notes: s.notes,
    viewport: `${W}x${H}`,
  };
}

/// The interpolated state at the current playback position. Fractions are always
/// present (both fidelities); exact counts only when the record carried them.
function stateAt(): {
  mFrac: number; dFrac: number; supp: number; engagedFrac: number;
  marines: number | null; defenders: number | null; notes: GroundNote[];
} {
  const empty = { mFrac: 0, dFrac: 0, supp: 0, engagedFrac: 0, marines: null, defenders: null, notes: [] as GroundNote[] };
  if (!rec || rec.rounds.length === 0) return empty;
  const i = Math.max(0, Math.min(round, rec.rounds.length - 1));
  const a = rec.rounds[i];
  const b = rec.rounds[Math.min(i + 1, rec.rounds.length - 1)];
  const t = Math.max(0, Math.min(1, frac));
  const lerp = (x: number, y: number) => x + (y - x) * t;
  const supp = lerp(a.suppression, b.suppression);
  const dFrac = lerp(a.defenders_frac, b.defenders_frac);
  const pick = (p: number | null, q: number | null) => (p === null || q === null ? p : Math.round(lerp(p, q)));
  return {
    mFrac: lerp(a.marines_frac, b.marines_frac),
    dFrac,
    supp,
    // What is actually in the firing line: pinned troops are in cover, and this
    // is the number that makes a broken blockade visible at a glance.
    engagedFrac: dFrac * (1 - supp),
    marines: pick(a.marines, b.marines),
    defenders: pick(a.defenders, b.defenders),
    notes: a.notes ?? [],
  };
}

const css = (name: string, fallback: string): string => {
  if (typeof getComputedStyle !== "function") return fallback;
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
};

function draw(): void {
  if (!ctx || !canvas || !rec) return;
  const g = ctx;
  const s = stateAt();
  const accent = css("--accent", "#6cf");
  const bad = css("--negative", "#e56");
  const dim = css("--dim", "#8a93a6");
  const portrait = W < 620;
  // Desktop keeps the original 900x380 composition. Portrait gets a deeper
  // battlefield and narrower troop blocks so neither the masses nor the
  // strength history are miniaturised into an unreadable landscape postcard.
  const horizon = portrait ? Math.min(160, H * 0.36) : H * (150 / DEFAULT_H);
  const lzX = portrait ? W * 0.24 : W * (200 / DEFAULT_W);
  const lineX = portrait ? W * 0.76 : W * (700 / DEFAULT_W);

  g.clearRect(0, 0, W, H);

  // --- SKY: dark, with the blockade's fire coming down through it -------------
  const sky = g.createLinearGradient(0, 0, 0, horizon);
  sky.addColorStop(0, "#070b14");
  sky.addColorStop(1, "#121a2b");
  g.fillStyle = sky;
  g.fillRect(0, 0, W, horizon);

  // ORBITAL BOMBARDMENT — streaks falling on the garrison line, in proportion to
  // suppression. When a blockade breaks, this visibly stops, and the defenders'
  // line fills back in. That is the whole story of the mechanic, shown rather
  // than captioned.
  if (s.supp > 0.02) {
    const n = Math.round(s.supp * 26);
    g.lineWidth = 2;
    for (let i = 0; i < n; i++) {
      const speed = 90 + scatter(i, 3) * 140;
      const p = (clock * speed + scatter(i, 1) * 400) % (horizon + 60);
      const spread = portrait ? W * 0.32 : 260;
      const x = lineX - spread * 0.58 + scatter(i, 2) * spread;
      const alpha = 0.25 + 0.5 * s.supp;
      g.strokeStyle = `rgba(255, 190, 120, ${alpha})`;
      g.beginPath();
      g.moveTo(x, p - 26);
      g.lineTo(x + 5, p);
      g.stroke();
    }
    // Impact glow on the line itself.
    const glowR = portrait ? W * 0.34 : 190;
    const glow = g.createRadialGradient(lineX, horizon, 4, lineX, horizon, glowR);
    glow.addColorStop(0, `rgba(255, 170, 90, ${0.10 + 0.26 * s.supp})`);
    glow.addColorStop(1, "rgba(255, 170, 90, 0)");
    g.fillStyle = glow;
    g.fillRect(lineX - glowR, horizon - 90, glowR * 2, 180);
  }

  // --- GROUND -----------------------------------------------------------------
  const ground = g.createLinearGradient(0, horizon, 0, H);
  ground.addColorStop(0, "#1a1410");
  ground.addColorStop(1, "#0d0b09");
  g.fillStyle = ground;
  g.fillRect(0, horizon, W, H - horizon);
  g.strokeStyle = "rgba(255,255,255,0.10)";
  g.lineWidth = 1;
  g.beginPath();
  g.moveTo(0, horizon);
  g.lineTo(W, horizon);
  g.stroke();

  // --- THE TWO MASSES ---------------------------------------------------------
  // Dot counts are capped and SCALED: a dot is a proportion of the force, never
  // a headcount, so nothing here can imply a number the record didn't carry.
  // Sized so a FULL-STRENGTH block still clears the graph strip below — the two
  // must never overlap, or the fight's shape gets drawn through its own troops.
  const DOTS = portrait ? 48 : 60;
  const COLS = portrait ? 6 : 12;
  const ROW_H = portrait ? 10 : 12;
  const troops = (
    xc: number, frac0: number, color: string, alpha: number, salt: number, dug: boolean,
  ) => {
    const n = Math.round(DOTS * Math.max(0, Math.min(1, frac0)));
    g.fillStyle = color;
    for (let i = 0; i < n; i++) {
      const col = i % COLS;
      const row = Math.floor(i / COLS);
      const jx = (scatter(i, salt) - 0.5) * 7;
      const jy = (scatter(i, salt + 7) - 0.5) * 5;
      const x = xc + (col - (COLS - 1) / 2) * (portrait ? 10 : 12) + jx;
      const y = horizon + 42 + row * ROW_H + jy;
      g.globalAlpha = alpha;
      g.beginPath();
      g.arc(x, y, dug ? 3.1 : 3.6, 0, Math.PI * 2);
      g.fill();
    }
    g.globalAlpha = 1;
  };

  // DEFENDERS: the whole surviving garrison drawn faint (they are alive), with
  // the ENGAGED fraction drawn solid on top (they are shooting). The gap between
  // the two IS the suppression — you can see the line thin out as the guns work
  // and fill straight back in when they stop.
  troops(lineX, s.dFrac, bad, 0.22, 11, true);
  troops(lineX, s.engagedFrac, bad, 1, 11, true);
  // MARINES: everyone who is still on their feet is in the fight.
  troops(lzX, s.mFrac, accent, 1, 29, false);

  // --- LABELS -----------------------------------------------------------------
  g.font = "600 13px ui-sans-serif, system-ui, sans-serif";
  g.textAlign = "center";
  g.fillStyle = accent;
  const mLabel = s.marines !== null ? `LANDING FORCE · ${s.marines}` : "LANDING FORCE";
  g.fillText(mLabel, lzX, horizon + 28, W * 0.45);
  g.fillStyle = bad;
  const dLabel = s.defenders !== null ? `GARRISON · ${s.defenders}` : "GARRISON";
  g.fillText(dLabel, lineX, horizon + 28, W * 0.45);

  // The suppression readout, where the eye already is.
  if (s.supp > 0.02) {
    g.fillStyle = "rgba(255,190,120,0.95)";
    g.font = "600 12px ui-sans-serif, system-ui, sans-serif";
    g.fillText(`${Math.round(s.supp * 100)}% PINNED IN COVER`, lineX, horizon - 14, W * 0.48);
  }

  // --- THE STRENGTH GRAPH -----------------------------------------------------
  // The fight's whole shape on one strip: both sides over time, with the pinned
  // band shaded. This is where "it turned here" is actually readable.
  const gx = portrait ? 18 : W * (40 / DEFAULT_W);
  const gh = portrait ? Math.min(66, H * 0.17) : H * (48 / DEFAULT_H);
  const gy = H - gh - 20;
  const gw = W - gx * 2;
  g.strokeStyle = "rgba(255,255,255,0.10)";
  g.strokeRect(gx, gy, gw, gh);
  const n = rec.rounds.length;
  if (n > 1) {
    const px = (i: number) => gx + (i / (n - 1)) * gw;
    const py = (v: number) => gy + gh - Math.max(0, Math.min(1, v)) * gh;
    const series = (get: (r: GroundRoundView) => number, color: string, fill: boolean) => {
      g.beginPath();
      rec!.rounds.forEach((r, i) => (i === 0 ? g.moveTo(px(i), py(get(r))) : g.lineTo(px(i), py(get(r)))));
      if (fill) {
        g.lineTo(px(n - 1), gy + gh);
        g.lineTo(px(0), gy + gh);
        g.closePath();
        g.fillStyle = color;
        g.fill();
      } else {
        g.strokeStyle = color;
        g.lineWidth = 2;
        g.stroke();
      }
    };
    // The pinned band: total garrison, with the engaged part drawn over it.
    series((r) => r.defenders_frac, "rgba(229,85,102,0.16)", true);
    series((r) => r.defenders_frac * (1 - r.suppression), "rgba(229,85,102,0.34)", true);
    series((r) => r.defenders_frac, bad, false);
    series((r) => r.marines_frac, accent, false);

    // BEATS: the derived notes, marked where they happened.
    rec.rounds.forEach((r, i) => {
      for (const note of r.notes ?? []) {
        g.strokeStyle = note === "tipped" ? "rgba(255,255,255,0.75)" : "rgba(255,190,120,0.75)";
        g.setLineDash(note === "tipped" ? [] : [3, 3]);
        g.lineWidth = 1;
        g.beginPath();
        g.moveTo(px(i), gy);
        g.lineTo(px(i), gy + gh);
        g.stroke();
        g.setLineDash([]);
      }
    });

    // The playhead, and the light frontier beyond it.
    const head = px(Math.max(0, Math.min(round + frac, n - 1)));
    g.strokeStyle = "rgba(255,255,255,0.9)";
    g.lineWidth = 1.5;
    g.beginPath();
    g.moveTo(head, gy - 4);
    g.lineTo(head, gy + gh + 4);
    g.stroke();
    if (live) {
      g.fillStyle = "rgba(255,255,255,0.06)";
      g.fillRect(px(n - 1), gy, gx + gw - px(n - 1), gh);
    }
  }

  // --- BANNERS ----------------------------------------------------------------
  g.textAlign = "left";
  g.font = "600 12px ui-sans-serif, system-ui, sans-serif";
  g.fillStyle = dim;
  g.fillText(noteCaption(s.notes, rec, live), gx, gy - 12, gw);

  if (rec.outcome && round >= rec.rounds.length - 1) {
    g.textAlign = "center";
    g.font = "700 24px ui-sans-serif, system-ui, sans-serif";
    g.fillStyle = rec.outcome === "taken" ? accent : bad;
    g.fillText(rec.outcome === "taken" ? "GROUND TAKEN" : "LANDING DESTROYED", W / 2, 46);
  }
}

/// The one-line caption under the graph: names the beat if this round has one,
/// otherwise says plainly what is happening.
function noteCaption(notes: GroundNote[], r: GroundRecordView, isLive: boolean): string {
  if (notes.includes("tipped")) return "THE LEAD CHANGES HANDS";
  if (notes.includes("guns_lifted")) return "THE GUNS HAVE STOPPED — pinned troops are rejoining the line";
  if (notes.includes("guns_resumed")) return "BOMBARDMENT RESUMES — the garrison is pinned again";
  if (notes.includes("garrison_starved")) return "THE GARRISON IS OUT OF PROVISIONS — it has stopped counting";
  if (isLive) return "LANDING IN PROGRESS — later rounds are still outside your light cone";
  const who = r.attacking ? "Your landing" : "A landing on your ground";
  return `${who} at ${Math.round(r.suppression_at_drop * 100)}% suppression, against ${r.garrison_tiers} garrison tier${r.garrison_tiers === 1 ? "" : "s"}`;
}

// §signals: the Signals sheet on mobile (docs/signals.md): the same cards as
// the Deck's pop-out, stacked, each with its painting as a banner. A tap opens
// the signal in place, with its Send rows or its report.
import {
  FIRST_TEN, SIGNAL_INFO, findRows, firstTenStrip, sendRows, signalCardArt, signalCards,
  signalsSummary, type SignalCard,
} from "../../core/derive/signals";
import { shipKindLabel } from "../../core/derive/fleet";
import { traderOf } from "../../core/derive/trader";
import { systemName } from "../../core/derive/geo";
import type { SignalKind, SignalView } from "../../protocol";
import { liveSimTime } from "../../state";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetStack, SheetView } from "./sheets";

interface SignalsHooks {
  notice(html: string): void;
  focus(pos: { x: number; y: number }): void;
}

// Each painting is probed once; until it loads, its card shows the line icon.
const probed = new Map<string, boolean>();
function painted(src: string, onLoad: () => void): boolean {
  if (probed.has(src)) return probed.get(src)!;
  if (typeof Image === "undefined") return false;
  probed.set(src, false);
  const image = new Image();
  image.onload = () => { probed.set(src, true); onLoad(); };
  image.src = src;
  return false;
}

export class MobileSignals {
  /** The signal open in the sheet; null lists them all. */
  private open: string | null = null;
  private appliedTarget = "";
  private rendered = "";

  constructor(
    private readonly ctx: CoreContext,
    private readonly sheets: SheetStack,
    private readonly hooks: SignalsHooks,
  ) {}

  render(entry: SheetEntry): SheetView | null {
    if (entry.id !== "signals") return null;
    this.applyTarget(entry);
    this.rendered = this.signature(entry);
    const st = this.ctx.state;
    const view = st.signals;
    if (!view) return { title: "Signals", html: `<div class="m-empty">No signals yet.</div>` };
    const now = liveSimTime();
    const cards = signalCards(view, st.ghosts, st.commandCenter, now, traderOf(st.founding));
    const chosen = this.open ? cards.find((c) => c.signal.id === this.open) : undefined;
    if (!chosen) this.open = null;
    const strip = firstTenStrip(view);
    const head = strip ? `<div class="m-signals-ten"><span>First ten · <b>${strip.done}/${FIRST_TEN}</b>${strip.next ? ` · next: ${esc(strip.next)}` : ""}</span>`
      + `<div class="m-signals-bar" aria-hidden="true">${Array.from({ length: FIRST_TEN }, (_, i) => `<i${i < strip.done ? ` class="is-done"` : ""}></i>`).join("")}</div></div>` : "";
    const html = chosen ? this.detail(chosen, now)
      : head + (cards.length ? cards.map((c) => this.card(c)).join("") : `<div class="m-empty">No signals yet.</div>`);
    return { title: chosen ? chosen.info.name : "Signals", eyebrow: signalsSummary(view, now), html, detent: "full" };
  }

  refreshNeeded(entry: SheetEntry): boolean | null {
    return entry.id === "signals" ? this.signature(entry) !== this.rendered : null;
  }

  handleClick(event: Event): boolean {
    const button = (event.target as Element).closest<HTMLButtonElement>("[data-mobile-act^='signal-']");
    if (!button) return false;
    const signal = button.dataset.signal ?? "";
    switch (button.dataset.mobileAct) {
      case "signal-open": this.open = signal; break;
      case "signal-list": this.open = null; break;
      case "signal-send": {
        const fleet = button.dataset.fleet ?? "";
        this.ctx.send({ type: "AnswerSignal", fleet_id: fleet, signal_id: signal });
        const g = this.ctx.state.ghosts.find((candidate) => candidate.id === fleet);
        this.hooks.notice(`<b>Sent</b> · ${esc(g ? shipKindLabel(g.kind) : "Fleet")}${g?.captain ? ` · ${esc(g.captain.name)}` : ""}.`);
        break;
      }
      case "signal-dismiss":
        this.ctx.send({ type: "DismissSignal", signal_id: signal });
        this.open = null;
        break;
      case "signal-map": {
        const pos = this.ctx.state.signals?.signals.find((s) => s.id === signal)?.pos;
        if (pos) this.hooks.focus(pos);
        break;
      }
      default:
        return false;
    }
    this.sheets.refresh();
    return true;
  }

  /** A map tap opens its signal; otherwise the sheet lands on the list. */
  private applyTarget(entry: SheetEntry): void {
    const target = (entry.props as { id?: string } | undefined)?.id ?? "";
    if (target === this.appliedTarget) return;
    this.appliedTarget = target;
    if (target) this.open = target;
  }

  private signature(entry: SheetEntry): string {
    const st = this.ctx.state;
    const own = st.ghosts.filter((g) => g.own).map((g) => [g.id, g.expedition, g.survey_progress, g.docked,
      Math.round(g.pos.x / 100), Math.round(g.pos.y / 100)]);
    return sheetFingerprint([entry.props ?? null, this.open, st.signals, own, st.captains, Math.floor(liveSimTime())]);
  }

  private art(kind: SignalKind): string {
    const art = signalCardArt(kind);
    if (painted(art.src, () => this.sheets.refresh())) {
      return `<img src="${art.src}" srcset="${art.srcset}" sizes="100vw" alt="" decoding="async">`;
    }
    return `<svg width="44" height="44" viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="1.3" aria-hidden="true">${SIGNAL_INFO[kind].icon}</svg>`;
  }

  private card(c: SignalCard): string {
    const s = c.signal;
    const tagClass = c.status === "reported" ? " is-report" : s.hazardous ? " is-hazard" : "";
    const progress = c.status === "answering"
      ? `<i class="m-signal-progress" aria-hidden="true"><i style="width:${Math.round((c.progress ?? 0) * 100)}%"></i></i>` : "";
    return `<button type="button" class="m-signal-card is-${c.status}${s.hazardous ? " is-hazardous" : ""}" data-mobile-act="signal-open" data-signal="${esc(s.id)}">`
      + `<span class="m-signal-card__art">${this.art(s.kind)}</span>`
      + `<span class="m-signal-card__meta"><span><b>${esc(c.info.name)}</b><em class="m-signal-tag${tagClass}">${esc(c.tag)}</em></span>`
      + `<small${c.status === "answering" ? ` class="is-live"` : ""}>${esc(c.line)}</small>${progress}</span></button>`;
  }

  private detail(c: SignalCard, now: number): string {
    const back = `<button type="button" class="m-signal-back" data-mobile-act="signal-list">← All signals</button>`;
    return back + `<div class="m-signal-card__art m-signal-hero">${this.art(c.signal.kind)}</div>`
      + (c.signal.report ? this.report(c.signal, now) : this.openSignal(c, now));
  }

  private openSignal(c: SignalCard, now: number): string {
    const st = this.ctx.state;
    const s = c.signal;
    const home = st.systems.find((sys) => sys.bodies?.some((b) => (b.structures?.command_center ?? 0) > 0))?.id;
    const place = (berth: string) => berth === "hub" ? "the Hub" : berth === home ? "home" : systemName(berth);
    const rows = sendRows(s, st.captains, st.ghosts, st.commandCenter, now, place);
    const hold = [...c.info.mayHold, ...(s.hazardous ? ["Experience ×2"] : [])].map((h) => `<span>${esc(h)}</span>`).join("");
    const send = rows.length ? rows.map((r) => {
      const note = r.blocked ? `<small${/Recovering/.test(r.blocked) ? ` class="is-warn"` : ""}>${esc(r.blocked)}</small>`
        : `<small>${r.risk ? `<span class="is-warn">${esc(r.risk)}</span> · ` : ""}${esc(r.trip ?? "")}</small>`;
      return `<div class="m-signal-send"><span><b>${esc(r.title)}</b><small>${esc(r.sub)}</small>${note}</span>`
        + `<button type="button" class="m-primary" data-mobile-act="signal-send" data-signal="${esc(s.id)}" data-fleet="${esc(r.fleet?.id ?? "")}" ${r.blocked || !r.fleet ? "disabled" : ""}>Send</button></div>`;
    }).join("") : `<div class="m-empty">No captains yet.</div>`;
    return `<div class="m-signal-title"><em class="m-signal-tag${s.hazardous ? " is-hazard" : ""}">${s.hazardous ? "Hazardous" : "Routine"}</em><span>${esc(c.line)}</span>`
      + `<button type="button" data-mobile-act="signal-map" data-signal="${esc(s.id)}">Map</button></div>`
      + `<p class="m-signal-blurb">${esc(c.info.blurb)}</p><div class="m-signal-chips">${hold}</div>`
      + (s.hazardous ? `<div class="m-signal-hurt">If hurt: captain out 4h</div>` : "")
      + `<section class="m-section"><h3>Send</h3>${send}</section><p class="m-muted">Rewards arrive with the report.</p>`;
  }

  private report(s: SignalView, now: number): string {
    const st = this.ctx.state;
    const report = s.report!;
    const fleet = st.ghosts.find((g) => g.own && g.id === report.fleet);
    const programme = (id: string) => st.research?.programmes.find((p) => p.id === id)?.name ?? id;
    const rows = findRows(s, fleet ? shipKindLabel(fleet.kind) : null, now, programme)
      .map((r) => `<div class="m-signal-found${r.tone ? ` is-${r.tone}` : ""}"><span>${esc(r.label)}</span><b>${esc(r.value)}</b></div>`).join("");
    return `<div class="m-signal-title"><em class="m-signal-tag is-report">Report in</em><span>Explored · ${esc(report.captain)}</span></div>`
      + `<section class="m-section"><h3>Found</h3>${rows}</section>`
      + `<button type="button" class="m-signal-dismiss" data-mobile-act="signal-dismiss" data-signal="${esc(s.id)}">Dismiss</button>`;
  }
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);
}

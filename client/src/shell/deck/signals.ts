// §signals: the Signals pop-out (docs/signals.md), a Market-style panel. The
// cards sit on the left with their paintings; the chosen signal sits on the
// right: its painting, what it may hold, and a Send row per captain. After the
// report, the same place lists what was found.
import {
  FIRST_TEN, SIGNAL_INFO, defaultSignal, findRows, firstTenStrip, sendRows, signalCardArt, signalCards, signalInfo,
  signalFrameArt, signalsSummary, signalDur, type SignalCard,
} from "../../core/derive/signals";
import { shipKindLabel } from "../../core/derive/fleet";
import { traderOf } from "../../core/derive/trader";
import { systemName } from "../../core/derive/geo";
import type { EntityId, SignalKind, SignalView } from "../../protocol";
import { liveSimTime } from "../../state";
import { captainPortrait } from "../art";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

interface SignalsHooks {
  go(route: DeckRoute): void;
  notice(html: string): void;
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

export class DeckSignalsRoutes {
  private selected: EntityId | null = null;
  private appliedTarget = "";
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: SignalsHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "signals") return false;
    const target = route.query?.signal ?? "";
    if (target !== this.appliedTarget) {
      this.appliedTarget = target;
      if (target) this.selected = target;
    }
    const st = this.ctx.state;
    const own = st.ghosts.filter((g) => g.own).map((g) => [g.id, g.expedition, g.survey_progress, g.docked,
      Math.round(g.pos.x / 100), Math.round(g.pos.y / 100), g.captain?.level, g.captain?.attributes.fieldcraft]);
    const signature = sheetFingerprint([route, this.selected, st.signals, own, st.captains, Math.floor(liveSimTime())]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, this.html());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "signals") return false;
    const signal = button.dataset.signal ?? "";
    switch (button.dataset.deckAct) {
      case "signal-select":
        this.selected = signal;
        break;
      case "signal-send": {
        const fleet = button.dataset.fleet ?? "";
        this.ctx.send({ type: "AnswerSignal", fleet_id: fleet, signal_id: signal });
        const g = this.ctx.state.ghosts.find((candidate) => candidate.id === fleet);
        this.hooks.notice(`<b>Sent</b> · ${esc(g ? shipKindLabel(g.kind) : "Fleet")}${g?.captain ? ` · ${esc(g.captain.name)}` : ""}.`);
        break;
      }
      case "signal-dismiss":
        this.ctx.send({ type: "DismissSignal", signal_id: signal });
        this.selected = null;
        break;
      case "signal-center": {
        const pos = this.ctx.state.signals?.signals.find((s) => s.id === signal)?.pos;
        if (pos) this.ctx.renderer.centerOnWorld(pos);
        break;
      }
      case "signal-cache": {
        const site = this.ctx.state.explorationSites.find((s) => s.id === button.dataset.site);
        if (site) this.ctx.renderer.centerOnWorld(site.pos);
        break;
      }
      case "signal-fleet":
        this.hooks.go({ name: "fleet", params: { id: button.dataset.fleet ?? "" } });
        return true;
      default:
        return false;
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  invalidate(): void {
    this.signature = "";
  }

  private html(): string {
    const st = this.ctx.state;
    const view = st.signals;
    if (!view) {
      return `<section class="deck-page deck-signals"><div class="deck-empty-inline">No signals yet.</div></section>`;
    }
    const now = liveSimTime();
    const cards = signalCards(view, st.ghosts, st.commandCenter, now, traderOf(st.founding));
    if (!this.selected || !cards.some((c) => c.signal.id === this.selected)) this.selected = defaultSignal(cards);
    const strip = firstTenStrip(view);
    const head = `<header class="deck-signals__head"><span>${esc(signalsSummary(view, now))}</span>`
      + (strip ? `<div class="deck-signals__ten"><span>First ten · <b>${strip.done}/${FIRST_TEN}</b></span>`
        + `<div class="deck-signals__bar" aria-hidden="true">${Array.from({ length: FIRST_TEN }, (_, i) => `<i${i < strip.done ? ` class="is-done"` : ""}></i>`).join("")}</div>`
        + (strip.next ? `<span>next: ${esc(strip.next)}</span>` : "") + `</div>` : "")
      + `</header>`;
    const next = view.next_at == null
      ? `<div class="deck-signal-next"><b>Slots full</b><small>Answer one for the next</small></div>`
      : `<div class="deck-signal-next"><b>Next signal</b><small>~${esc(signalDur(Math.max(0, view.next_at - now)))}</small></div>`;
    const list = `<div class="deck-signals__cards">${cards.map((c) => this.card(c)).join("")}${next}</div>`;
    const chosen = cards.find((c) => c.signal.id === this.selected);
    const detail = chosen ? this.detail(chosen, now) : `<div class="deck-signals__detail"><div class="deck-empty-inline">No signals yet.</div></div>`;
    return `<section class="deck-page deck-signals">${head}<div class="deck-signals__body">${list}${detail}</div></section>`;
  }

  private art(kind: SignalKind, frame: boolean): string {
    const art = frame ? signalFrameArt(kind) : signalCardArt(kind);
    const info = SIGNAL_INFO[kind];
    if (painted(art.src, () => { this.signature = ""; })) {
      return `<img src="${art.src}" srcset="${art.srcset}" sizes="${frame ? "640px" : "280px"}" alt="" decoding="async">`;
    }
    return `<svg width="${frame ? 72 : 40}" height="${frame ? 72 : 40}" viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="${frame ? 1 : 1.3}" aria-hidden="true">${info.icon}</svg>`;
  }

  private card(c: SignalCard): string {
    const s = c.signal;
    const tagClass = c.status === "reported" ? " is-report" : s.hazardous ? " is-hazard" : "";
    const progress = c.status === "answering"
      ? `<i class="deck-signal-progress" aria-hidden="true"><i style="width:${Math.round((c.progress ?? 0) * 100)}%"></i></i>` : "";
    return `<button type="button" class="deck-signal-card${s.id === this.selected ? " is-selected" : ""} is-${c.status}${s.hazardous ? " is-hazardous" : ""}" data-deck-act="signal-select" data-signal="${esc(s.id)}" aria-pressed="${s.id === this.selected}">`
      + `<span class="deck-signal-card__art">${this.art(s.kind, false)}</span>`
      + `<span class="deck-signal-card__meta"><span><b>${esc(c.info.name)}</b><em class="deck-signal-tag${tagClass}">${esc(c.tag)}</em></span>`
      + `<small${c.status === "answering" ? ` class="is-live"` : ""}>${esc(c.line)}</small>${progress}</span></button>`;
  }

  private detail(c: SignalCard, now: number): string {
    return c.signal.report ? this.report(c.signal, now) : this.open(c, now);
  }

  private open(c: SignalCard, now: number): string {
    const st = this.ctx.state;
    const s = c.signal;
    const home = st.systems.find((sys) => sys.bodies?.some((b) => (b.structures?.command_center ?? 0) > 0))?.id;
    const place = (berth: string) => berth === "hub" ? "the Hub" : berth === home ? "home" : systemName(berth);
    const rows = sendRows(s, st.captains, st.ghosts, st.commandCenter, now, place);
    const hold = [...c.info.mayHold, ...(s.hazardous ? ["Experience ×2"] : [])]
      .map((h) => `<span>${esc(h)}</span>`).join("");
    const send = rows.length ? rows.map((r) => {
      const portrait = r.fleet?.captain ?? r.captain.report;
      const face = portrait
        ? captainPortrait(r.captain.portrait, portrait.portrait_age, r.captain.name, "deck-signal-send__face", true)
        : `<span class="deck-signal-send__face is-initials">${esc(initials(r.captain.name))}</span>`;
      const note = r.blocked
        ? `<small class="deck-signal-send__note${/Recovering/.test(r.blocked) ? " is-warn" : ""}">${esc(r.blocked)}</small>`
        : `<small class="deck-signal-send__note">${r.risk ? `<span class="is-warn">${esc(r.risk)}</span> · ` : ""}${esc(r.trip ?? "")}</small>`;
      return `<div class="deck-signal-send__row">${face}<span class="deck-signal-send__who"><b>${esc(r.title)}</b><small>${esc(r.sub)}</small></span>${note}`
        + `<button type="button" class="is-primary" data-deck-act="signal-send" data-coach="signal-send" data-signal="${esc(s.id)}" data-fleet="${esc(r.fleet?.id ?? "")}" ${r.blocked || !r.fleet ? "disabled" : ""}>Send</button></div>`;
    }).join("") : `<div class="deck-empty-inline">No captains yet.</div>`;
    return `<div class="deck-signals__detail">`
      + `<figure class="deck-signal-detail__art">${this.art(s.kind, true)}</figure>`
      + `<div class="deck-signal-detail__title"><h2>${esc(c.info.name)}</h2><em class="deck-signal-tag${s.hazardous ? " is-hazard" : ""}">${s.hazardous ? "Hazardous" : "Routine"}</em>`
      + `<span>${esc(c.line)}</span><button type="button" data-deck-act="signal-center" data-signal="${esc(s.id)}">Center on map</button></div>`
      + `<p class="deck-signal-detail__blurb">${esc(c.info.blurb)}</p>`
      + `<div class="deck-signal-detail__hold"><div class="deck-signal-chips" title="May hold">${hold}</div>`
      + (s.hazardous ? `<span class="deck-signal-hurt">If hurt: captain out 4h</span>` : "") + `</div>`
      + `<section class="deck-signal-send" aria-label="Send"><h3>Send</h3>${send}</section>`
      + `<small class="deck-signal-detail__foot">Rewards arrive with the report.</small></div>`;
  }

  private report(s: SignalView, now: number): string {
    const st = this.ctx.state;
    const report = s.report!;
    const info = signalInfo(s.kind, traderOf(st.founding));
    const fleet = st.ghosts.find((g) => g.own && g.id === report.fleet);
    const ship = fleet ? shipKindLabel(fleet.kind) : null;
    const programme = (id: string) => st.research?.programmes.find((p) => p.id === id)?.name ?? id;
    const rows = findRows(s, ship, now, programme);
    const found = rows.map((r) => `<div class="deck-signal-found${r.tone ? ` is-${r.tone}` : ""}"><span>${esc(r.label)}</span><b>${esc(r.value)}</b></div>`).join("");
    const cache = report.finds.find((f) => f.kind === "cache");
    const actions = [
      fleet ? `<button type="button" data-deck-act="signal-fleet" data-signal="${esc(s.id)}" data-fleet="${esc(fleet.id)}">Select ${esc(ship!)}</button>` : "",
      cache?.kind === "cache" ? `<button type="button" data-deck-act="signal-cache" data-signal="${esc(s.id)}" data-site="${esc(cache.site)}">Show the cache</button>` : "",
      `<button type="button" data-deck-act="signal-dismiss" data-signal="${esc(s.id)}">Dismiss</button>`,
    ].join("");
    const first = rows[0];
    const lead = !report.finds.length ? "Nothing of value"
      : first.label === "Credits" ? first.value
      : /^(aboard|left|in reserve|flying)/.test(first.value) ? `${first.label} ${first.value}` : first.label;
    const headline = `${lead}${report.finds.length > 1 ? ` · +${report.finds.length - 1} more` : ""} · +${report.xp} XP`;
    return `<div class="deck-signals__detail">`
      + `<div class="deck-signal-report__banner" role="status"><b>Report in</b><span>${esc(headline)}</span></div>`
      + `<figure class="deck-signal-detail__art">${this.art(s.kind, true)}</figure>`
      + `<div class="deck-signal-detail__title"><h2>${esc(info.name)}</h2><span>Explored · ${esc(report.captain)}</span></div>`
      + `<section class="deck-signal-found-list" aria-label="Found"><h3>Found</h3>${found}</section>`
      + `<div class="deck-signal-detail__actions">${actions}</div></div>`;
  }
}

function initials(name: string): string {
  return name.split(/\s+/).map((w) => w[0] ?? "").join("").slice(0, 2).toUpperCase();
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);
}

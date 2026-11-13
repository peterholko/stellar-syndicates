// §shipyard-view (Peter, Oct 21 2026): the Shipyard pop-out. A hull under
// construction stands in its yard's interior (ship-construction-view.ts),
// built from the stern as the served report's progress rises. It opens when a
// ship is queued, and from a hull line in the Construction card.
import "../../ship-construction-view";
import { liveProgressAttr } from "../../core/derive/construction";
import { pickYardJob, yardFallback, yardJobs, yardRepairs, type YardJob } from "../../core/derive/shipyard";
import { shipKindLabel } from "../../core/derive/fleet";
import { hullFigures } from "../../core/derive/orders";
import { commandDelayTo } from "../../core/derive/geo";
import { YARD_TITLE } from "../../core/derive/market";
import { SENDING } from "../../core/derive/worldbuild";
import { HULL_YARD } from "../../shipyard-scene";
import type { ShipKind } from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

interface ShipyardHooks {
  go(route: DeckRoute): void;
}

export class DeckShipyardRoutes {
  private picked: string | null = null;
  private signature = "";
  /** The route last drawn, so a newly opened yard starts at its top. */
  private shown = "";
  /** Hulls the yard was opened on that a report has shown building. */
  private readonly seen = new Set<string>();

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: ShipyardHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "shipyard") return false;
    const system = this.ctx.state.systems.find((entry) => entry.id === route.params?.id);
    const repairs = yardRepairs(this.ctx.state.ghosts, system).map((job) => [job.key, job.progress]);
    const signature = sheetFingerprint([route, this.picked, Math.floor(liveSimTime()), system?.builds, system?.bodies.length, repairs]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, this.html(route));
    const shown = JSON.stringify([route.params?.id, route.query?.kind, route.query?.repair]);
    if (shown !== this.shown) {
      this.shown = shown;
      this.root.scrollTop = 0;
    }
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "shipyard") return false;
    const act = button.dataset.deckAct;
    if (act === "shipyard-pick") {
      this.picked = button.dataset.job ?? null;
      this.render(route, true);
      return true;
    }
    if (act === "shipyard-build") {
      const id = route.params?.id;
      if (id) this.hooks.go({ name: "build", params: { systemId: id, systemLabel: route.params?.systemLabel ?? "System" }, query: { mode: "ships" } });
      return true;
    }
    return false;
  }

  invalidate(): void {
    this.signature = "";
  }

  private html(route: DeckRoute): string {
    const id = route.params?.id;
    const system = this.ctx.state.systems.find((entry) => entry.id === id);
    const now = liveSimTime();
    // §yard-repair: hulls in for repairs follow the builds.
    const jobs = [...yardJobs(system, now), ...yardRepairs(this.ctx.state.ghosts, system)];
    const ordered = route.query?.kind && HULL_YARD[route.query.kind as ShipKind] ? route.query.kind as ShipKind : null;
    // The hull the yard opened on keeps the bay unless a tab is picked: an
    // order just sent shows at once, as a blueprint, until its report arrives;
    // seen building and then gone, it launched.
    const picked = jobs.find((entry) => entry.key === this.picked)
      ?? (route.query?.repair ? jobs.find((entry) => entry.repair?.fleet === route.query?.repair) : undefined) ?? null;
    const watch = ordered ? `${id}|${ordered}|${route.query?.sent ?? ""}` : null;
    const inYard = jobs.some((entry) => entry.kind === ordered);
    if (watch && inYard) this.seen.add(watch);
    const sentAt = route.query?.sent ? Number(route.query.sent) : NaN;
    const pos = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id)?.pos;
    const fallback = !picked && watch && !inYard
      ? yardFallback(this.seen.has(watch), Number.isFinite(sentAt) ? sentAt : null, pos ? commandDelayTo(pos) ?? 0 : 0, now)
      : null;
    const job = picked ?? (fallback ? null : pickYardJob(jobs, null, ordered));
    const build = `<button type="button" data-deck-act="shipyard-build">Build ships</button>`;
    if (!job && !fallback) {
      return `<section class="deck-page deck-shipyard"><div class="deck-empty-inline">No hulls in the yard.</div><div class="deck-shipyard__dock">${build}</div></section>`;
    }
    const kind = job?.kind ?? ordered!;
    const yard = job?.yard ?? HULL_YARD[kind]!;
    const progress = job?.repair ? 1 : job ? job.progress : fallback?.state === "launched" ? 1 : null;
    const tabs = jobs.length > 1 ? `<nav class="deck-shipyard__tabs" aria-label="Hulls in the yard">${jobs.map((entry) => tab(entry, entry === job)).join("")}</nav>` : "";
    const bar = job?.progress == null ? "" : `<span class="deck-queue-progress${job.repair ? " is-repair" : ""}" role="progressbar"${liveProgressAttr(job.line)} aria-label="${esc(job.title)} ${job.repair ? "hull" : "build progression"}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(job.progress * 100)}"><i style="width:${(job.progress * 100).toFixed(1)}%"></i></span>`;
    const note = fallback?.state === "silent"
      ? `<small class="is-warn">No report yet · it may have been refused</small>`
      : `<small>${esc(job?.where ?? YARD_TITLE[yard] ?? "Shipyard")}</small>`;
    const status = job ? job.status : fallback?.state === "launched" ? "Launched" : fallback?.state === "sending" ? SENDING : "";
    const info = `<div class="deck-shipyard__info"><div><b>${esc(job?.title ?? shipKindLabel(kind))}</b>${note}</div>${bar}<em>${esc(status)}</em></div>`;
    const view = `<ship-construction-view id="shipyard-view" kind="${esc(kind)}" yard="${yard}"${progress === null ? "" : ` progress="${progress.toFixed(4)}"`}></ship-construction-view>`;
    return `<section class="deck-page deck-shipyard">${tabs}${view}${info}${hullStatsHtml(kind)}<div class="deck-shipyard__dock">${build}</div></section>`;
  }
}

/** §hull stats (Peter, Oct 27 2026): the hull's figures under its 3D model,
 * here and in the ship builder. */
export function hullStatsHtml(kind: string): string {
  const figures = hullFigures(kind);
  return figures.length ? `<div class="deck-hull-stats">${figures.map(([name, value]) =>
    `<span class="deck-stat"><small>${esc(name)}</small><b>${esc(value)}</b></span>`).join("")}</div>` : "";
}

function tab(job: YardJob, current: boolean): string {
  const state = job.progress !== null ? `${Math.round(job.progress * 100)}%` : job.status;
  return `<button type="button" data-deck-act="shipyard-pick" data-job="${esc(job.key)}" aria-pressed="${current}"><b>${esc(job.title)}</b><small>${esc(state)}</small></button>`;
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);
}

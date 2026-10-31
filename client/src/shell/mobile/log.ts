// §log (Oct 28 2026): the phone's Log sheet, the desk's two tabs (shell/deck/
// log.ts) over the same models. Inbox rows carry one button and a menu;
// History rows open into their report. Every `Go` lands on a sheet or the map.

import { archiveRows } from "../../core/derive/battlearchive";
import { COMMODITY_VALUE } from "../../core/derive/fleet";
import { ageLabel, fmtBuildDur, gameClock } from "../../core/derive/format";
import { systemName } from "../../core/derive/geo";
import {
  dayLabel, goodsValue, HISTORY_FILTERS, historyRows, rowActions, unreadNewsCount, type HistoryFilter, type HistoryRow,
} from "../../core/derive/history";
import {
  computeInbox, INBOX_GROUPS, loadInboxPrefs, saveInboxPrefs, SNOOZE_S, visibleInbox, type Go, type InboxItem, type InboxPrefs,
} from "../../core/derive/inbox";
import { nextDecisionLabel } from "../../core/derive/orders";
import { freshNews, latestNews, newsRows, seasonLine } from "../../core/derive/season";
import { label } from "../../icons";
import type { Commodity, EntityId } from "../../protocol";
import { liveSimTime } from "../../state";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetView } from "./sheets";

interface MobileLogHooks {
  openSheet(entry: SheetEntry): void;
  focusSystem(id: string): void;
  focusFleet(id: string): void;
  notice(html: string): void;
  refresh(): void;
}

type LogTab = "inbox" | "history" | "news";

const INBOX_SHOWN = 5;
const valueOf = (good: Commodity): number => COMMODITY_VALUE[good] ?? 0;

export class MobileLog {
  private tab: LogTab = "inbox";
  private filter: HistoryFilter = "all";
  private system: EntityId | null = null;
  private showAll = false;
  private awayClosed = false;
  private readonly open = new Set<string>();
  private prefs: InboxPrefs = { dismissed: [], snoozed: {}, readThrough: null, seen: [] };
  /** The Inbox's seen marks when it opened, so new rows stay marked. */
  private seenAtOpen: Set<string> | null = null;
  private prefsFor: string | null = null;
  private readAtOpen: number | null = null;
  /** §season: the News tab's unread boundary while it is open. */
  private newsAtOpen: number | null = null;
  private opened = false;
  private appliedProps: unknown = undefined;

  constructor(private readonly ctx: CoreContext, private readonly hooks: MobileLogHooks) {}

  render(entry: SheetEntry): SheetView {
    this.applyProps(entry);
    const inbox = this.items();
    this.markTabRead();
    const rows = this.history();
    const unread = rows.filter((row) => row.unread).length;
    const news = freshNews(this.ctx.state.season, this.loadPrefs().newsRead ?? latestNews(this.ctx.state.season));
    const tabs = `<div class="m-subtabs m-subtabs--3" role="tablist">`
      + `<button type="button" data-mobile-act="log-tab" data-tab="inbox" aria-selected="${this.tab === "inbox"}">Inbox${inbox.length ? ` · ${inbox.length}` : ""}</button>`
      + `<button type="button" data-mobile-act="log-tab" data-tab="history" aria-selected="${this.tab === "history"}">History${unread ? ` · ${unread} new` : ""}</button>`
      + `<button type="button" data-mobile-act="log-tab" data-tab="news" aria-selected="${this.tab === "news"}">News${news ? ` · ${news} new` : ""}</button></div>`;
    const body = this.tab === "inbox" ? this.inboxHtml(inbox) : this.tab === "news" ? this.newsHtml() : this.historyHtml(rows);
    return { title: "Log", eyebrow: "Inbox · history · news", html: tabs + body };
  }

  /** §season: the Galactic News Network, newest first. */
  private newsHtml(): string {
    const season = this.ctx.state.season;
    const line = seasonLine(season);
    const head = line ? `<p class="m-news-head">${esc(line)}</p>` : "";
    const rows = newsRows(season, liveSimTime(), this.newsAtOpen);
    if (!rows.length) return `${head}<div class="m-muted">No news has reached you yet.</div>`;
    return head + rows.map((row) => {
      const go = row.fleet ? ` data-fleet="${esc(row.fleet)}"` : row.system ? ` data-system="${esc(row.system)}"` : "";
      const button = go ? `<button type="button" class="m-logrow__act" data-mobile-act="log-news-go"${go}>View</button>` : "";
      return `<article class="m-logrow m-newsrow is-${row.tone}${row.unread ? " is-unread" : ""}${row.yours ? " is-yours" : ""}${row.free ? " is-free" : ""}">`
        + `<img class="m-newsrow__source" src="${esc(row.emblem)}" alt="${esc(row.source)}" decoding="async">`
        + `<span class="m-logrow__text"><b>${esc(row.headline)}</b><small>${esc(row.meta)}</small></span>${button}</article>`;
    }).join("");
  }

  signature(entry: SheetEntry): string {
    const state = this.ctx.state;
    return sheetFingerprint([
      Math.floor(liveSimTime() / 5), entry.props ?? null, this.tab, this.filter, this.system, this.showAll, this.awayClosed,
      [...this.open].sort(), this.loadPrefs(), state.timeline, state.systems, state.ghosts, state.battles, state.battleReports,
      state.captureReports, state.battleRecords.map((record) => [record.id, record.outcome]), state.diplomacy,
      state.syndicateInvites, state.operations, state.standingOrders, state.welcome, state.season,
    ]);
  }

  /** The sheet closed: the next open picks its tab afresh. */
  closed(): void {
    this.readAtOpen = null;
    this.newsAtOpen = null;
    this.seenAtOpen = null;
    this.opened = false;
    this.appliedProps = undefined;
  }

  items(): InboxItem[] {
    return visibleInbox(computeInbox(this.ctx.state), this.loadPrefs(), liveSimTime(), this.ctx.state.systems.length > 0);
  }

  /** New inbox rows plus unread warn-or-worse news; `open` marks what shows. */
  badgeCount(open: boolean): number {
    if (open) this.markTabRead();
    const seen = this.loadPrefs().seen;
    const fresh = this.items().filter((item) => !seen.includes(item.key)).length;
    return fresh + unreadNewsCount(this.ctx.state.timeline, this.loadPrefs().readThrough ?? this.ctx.state.awaySince);
  }

  handleAction(button: HTMLElement): boolean {
    const { mobileAct, key = "", tab, filter } = button.dataset;
    switch (mobileAct) {
      case "log-tab":
        this.tab = tab === "history" ? "history" : tab === "news" ? "news" : "inbox";
        break;
      case "log-news-go": {
        const { system, fleet } = button.dataset;
        if (fleet) this.hooks.focusFleet(fleet);
        else if (system) this.hooks.focusSystem(system);
        return true;
      }
      case "log-filter":
        this.filter = HISTORY_FILTERS.some((entry) => entry.key === filter) ? filter as HistoryFilter : "all";
        break;
      case "log-system-clear":
        this.system = null;
        break;
      case "log-row":
        if (this.open.has(key)) this.open.delete(key); else this.open.add(key);
        break;
      case "log-go": {
        const index = Number(button.dataset.index ?? 0);
        const row = button.dataset.source === "row" ? this.history().find((candidate) => candidate.key === key) : undefined;
        const action = row ? rowActions(row, systemName)[index] : this.items().find((item) => item.key === key)?.actions[index];
        if (action) this.run(action.go);
        break;
      }
      case "log-dismiss":
        this.prefs.dismissed.push(key);
        saveInboxPrefs(this.ctx.state.playerId, this.prefs);
        break;
      case "log-snooze":
        this.prefs.snoozed[key] = liveSimTime() + SNOOZE_S;
        saveInboxPrefs(this.ctx.state.playerId, this.prefs);
        break;
      case "log-more":
        this.showAll = !this.showAll;
        break;
      case "log-away-close":
        this.awayClosed = true;
        break;
      default:
        return false;
    }
    this.hooks.refresh();
    return true;
  }

  private applyProps(entry: SheetEntry): void {
    if (!this.opened) {
      // A fresh open shows what needs you, else what happened.
      this.opened = true;
      this.tab = this.items().length ? "inbox" : "history";
    }
    // An entry's props apply once, when it is opened.
    if (entry.props === this.appliedProps) return;
    this.appliedProps = entry.props;
    const props = (entry.props && typeof entry.props === "object" ? entry.props : {}) as { tab?: string; filter?: string; system?: string };
    if (props.tab === "history" || props.tab === "inbox" || props.tab === "news") this.tab = props.tab;
    if (props.filter && HISTORY_FILTERS.some((candidate) => candidate.key === props.filter)) this.filter = props.filter as HistoryFilter;
    if (props.system) this.system = props.system;
  }

  private loadPrefs(): InboxPrefs {
    const player = this.ctx.state.playerId;
    if (player !== this.prefsFor) {
      this.prefs = loadInboxPrefs(player);
      this.prefsFor = player;
      this.seenAtOpen = null;
    }
    return this.prefs;
  }

  private markDecisionsRead(): void {
    const prefs = this.loadPrefs();
    const keys = this.items().map((item) => item.key);
    this.seenAtOpen ??= new Set(prefs.seen);
    const fresh = keys.filter((key) => !prefs.seen.includes(key));
    if (fresh.length) {
      prefs.seen.push(...fresh);
      saveInboxPrefs(this.ctx.state.playerId, prefs);
    }
  }

  private markTabRead(): void {
    if (this.tab === "history") this.markHistoryRead();
    else if (this.tab === "news") this.markNewsRead();
    else this.markDecisionsRead();
  }

  /** §season: News heard so far is read once the tab shows it. */
  private markNewsRead(): void {
    const prefs = this.loadPrefs();
    const latest = latestNews(this.ctx.state.season);
    this.newsAtOpen ??= prefs.newsRead ?? latest;
    if (latest > (prefs.newsRead ?? -Infinity)) {
      prefs.newsRead = latest;
      saveInboxPrefs(this.ctx.state.playerId, prefs);
    }
  }

  private markHistoryRead(): void {
    const prefs = this.loadPrefs();
    const latest = this.ctx.state.timeline.reduce((max, entry) => Math.max(max, entry.at_time), 0);
    this.readAtOpen ??= prefs.readThrough ?? this.ctx.state.awaySince;
    if (latest > (prefs.readThrough ?? -Infinity)) {
      prefs.readThrough = latest;
      saveInboxPrefs(this.ctx.state.playerId, prefs);
    }
  }

  private run(go: Go): void {
    const state = this.ctx.state;
    switch (go.to) {
      case "system": this.hooks.focusSystem(go.id); break;
      case "fleet": this.hooks.focusFleet(go.id); break;
      case "withdraw": this.ctx.intent.beginFleetCommand({ type: "Withdraw", fleet_id: go.fleet }); break;
      case "send":
        this.ctx.send(go.command);
        if (go.done) this.hooks.notice(esc(go.done));
        break;
      case "history":
        this.tab = "history";
        this.filter = go.filter ?? "all";
        this.system = go.system ?? null;
        break;
      case "showAll": this.showAll = true; break;
      case "panel": {
        const params = go.params ?? {};
        if (go.panel === "battle") {
          // Reports open the battle they came from; a capture has no replay.
          const id = params.report === "battle"
            ? state.battleReports.find((report) => String(report.id) === params.id)?.battle_id ?? null
            : params.report === "capture" ? null : params.id;
          if (id) this.hooks.openSheet({ id: "battle", props: { id } });
          else {
            this.tab = "history";
            this.filter = "combat";
          }
        } else if (go.panel === "production") this.hooks.focusSystem(params.id);
        else if (go.panel === "build") this.hooks.openSheet({ id: "build", props: { systemId: params.id } });
        else if (go.panel === "world") this.hooks.openSheet({ id: "planet", props: { systemId: params.systemId, bodyId: Number(params.bodyId) } });
        else if (go.panel === "comms") this.hooks.openSheet({ id: "comms", props: { tab: "inbox" } });
        else this.hooks.openSheet({ id: go.panel });
        break;
      }
    }
  }

  private history(): HistoryRow[] {
    const prefs = this.loadPrefs();
    return historyRows(this.ctx.state.timeline, {
      filter: this.filter, system: this.system, now: liveSimTime(),
      readThrough: this.readAtOpen ?? prefs.readThrough ?? this.ctx.state.awaySince,
      nameOf: systemName, valueOf,
    });
  }

  private inboxHtml(items: InboxItem[]): string {
    if (!items.length) return `<div class="m-empty m-empty--good">All clear · ${esc(nextDecisionLabel())}</div>`;
    const shown = this.showAll ? items : items.slice(0, INBOX_SHOWN);
    const groups = INBOX_GROUPS.map((group) => {
      const mine = shown.filter((item) => item.group === group.key);
      return mine.length ? `<h4 class="m-log-group">${group.label}<b>${items.filter((item) => item.group === group.key).length}</b></h4>${mine.map((item) => this.inboxRowHtml(item)).join("")}` : "";
    }).join("");
    const more = items.length > INBOX_SHOWN
      ? `<button type="button" class="m-log-more" data-mobile-act="log-more">${this.showAll ? "Show fewer" : `+${items.length - INBOX_SHOWN} more`}</button>` : "";
    return groups + more;
  }

  private inboxRowHtml(item: InboxItem): string {
    const [primary, ...rest] = item.actions;
    const key = esc(item.key);
    const button = primary
      ? `<button type="button" class="m-logrow__act${primary.danger ? " is-danger" : ""}" data-mobile-act="log-go" data-source="inbox" data-key="${key}" data-index="0">${esc(primary.label)}</button>` : "";
    const menu = `<details class="m-logrow__menu"><summary aria-label="More actions">⋯</summary><div>`
      + rest.map((action, index) => `<button type="button" data-mobile-act="log-go" data-source="inbox" data-key="${key}" data-index="${index + 1}">${esc(action.label)}</button>`).join("")
      + `<button type="button" data-mobile-act="log-snooze" data-key="${key}">Snooze 1h</button>`
      + `<button type="button" data-mobile-act="log-dismiss" data-key="${key}">Dismiss</button></div></details>`;
    return `<article class="m-logrow is-${item.tone}${this.seenAtOpen && !this.seenAtOpen.has(item.key) ? " is-unread" : ""}" title="${esc(item.hint)}">`
      + `<span class="m-logrow__text"><b>${esc(item.title)}</b>${item.facts ? `<small>${esc(item.facts)}</small>` : ""}</span>${button}${menu}</article>`;
  }

  private historyHtml(rows: HistoryRow[]): string {
    const now = liveSimTime();
    const filters = `<div class="m-log-filters" role="toolbar" aria-label="Filter history">${HISTORY_FILTERS.map((entry) =>
      `<button type="button" data-mobile-act="log-filter" data-filter="${entry.key}" aria-pressed="${entry.key === this.filter}">${entry.label}</button>`).join("")}`
      + (this.system ? `<button type="button" aria-pressed="true" data-mobile-act="log-system-clear" aria-label="Clear the system filter">${esc(systemName(this.system))} ✕</button>` : "") + `</div>`;
    let day = "";
    const body = rows.map((row) => {
      const header = dayLabel(row.at, now);
      const heading = header !== day ? `<h4 class="m-log-day">${esc(header)}</h4>` : "";
      day = header;
      return heading + this.historyRowHtml(row, now);
    }).join("");
    return `${this.awayHtml()}${filters}${this.filter === "combat" ? this.engagementsHtml() : ""}${body || `<div class="m-muted">Nothing here yet.</div>`}`;
  }

  private historyRowHtml(row: HistoryRow, now: number): string {
    const open = this.open.has(row.key);
    return `<article class="m-logrow is-${row.severity}${row.unread ? " is-unread" : ""}${open ? " is-open" : ""}">`
      + `<button type="button" class="m-logrow__main" data-mobile-act="log-row" data-key="${esc(row.key)}" aria-expanded="${open}">`
      + `<span class="m-logrow__text"><b>${esc(row.title)}</b>${row.detail ? `<small>${esc(row.detail)}</small>` : ""}</span>`
      + `<time>${esc(ageLabel(now - row.at))}</time></button>${open ? this.reportHtml(row) : ""}</article>`;
  }

  private reportHtml(row: HistoryRow): string {
    const lines = row.entries.length > 1 || row.title !== row.entries[0].text
      ? `<ol class="m-logreport__lines">${row.entries.map((entry) => `<li><time>${esc(gameClock(entry.at_time))}</time> ${esc(entry.text)}</li>`).join("")}</ol>` : "";
    const goods = row.goods.length
      ? `<table class="m-logreport__goods"><tbody>${row.goods.map(([good, n]) =>
        `<tr><td>${esc(label(good))}</td><td class="${n < 0 ? "is-neg" : ""}">${n > 0 ? "+" : "−"}${Math.abs(n).toLocaleString()}</td><td>${Math.round(Math.abs(n) * valueOf(good)).toLocaleString()} Cr</td></tr>`).join("")}`
        + `</tbody><tfoot><tr><td>Total</td><td></td><td>${Math.round(goodsValue(row.goods, valueOf)).toLocaleString()} Cr</td></tr></tfoot></table>` : "";
    const actions = rowActions(row, systemName).map((action, index) =>
      `<button type="button"${action.primary ? ` class="m-primary"` : ""} data-mobile-act="log-go" data-source="row" data-key="${esc(row.key)}" data-index="${index}">${esc(action.label)}</button>`).join("");
    return `<div class="m-logreport">${lines}${goods}${actions ? `<div class="m-chip-actions">${actions}</div>` : ""}</div>`;
  }

  /** §welcome (R9): the server's report on return; its next steps run
   * through the sheet's `welcome-go` (the phone has no welcome dialog). */
  private awayHtml(): string {
    const welcome = this.ctx.state.welcome;
    if (!welcome || this.awayClosed || (!welcome.happened.length && !welcome.next.length)) return "";
    const next = welcome.next.map((action, i) => {
      const id = action.go.kind === "system" ? action.go.id : action.go.kind === "shipyard" ? action.go.system : "";
      const body = action.go.kind === "shipyard" ? ` data-body="${action.go.body}"` : "";
      return `<button type="button"${i === 0 ? ` class="m-primary"` : ""} data-mobile-act="welcome-go" data-go="${action.go.kind}"${id ? ` data-id="${esc(id)}"` : ""}${body}>${esc(action.label)}</button>`;
    }).join("");
    return `<div class="m-log-away"><header><b>Away · ${esc(fmtBuildDur(welcome.away))}</b><button type="button" data-mobile-act="log-away-close" aria-label="Close the summary">✕</button></header>`
      + `<ul>${welcome.happened.slice(0, 4).map((line) => `<li class="is-${line.severity}">${esc(line.text)}</li>`).join("")}</ul>${next ? `<div class="m-chip-actions">${next}</div>` : ""}</div>`;
  }

  private engagementsHtml(): string {
    const rows = archiveRows(this.ctx.state, liveSimTime());
    if (!rows.length) return "";
    return `<h4 class="m-log-group">Engagements<b>${rows.length}</b></h4>` + rows.map((row) =>
      `<div class="m-battle-row is-${row.tone}"><span><b>${esc(row.where)}</b><small>${esc(row.ago)} · ${esc(row.sides)}</small></span><em>${esc(row.outcome)}</em>`
      + `<button type="button" data-mobile-act="battle-open" data-id="${esc(row.id)}">Open</button></div>`).join("");
  }
}

function esc(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

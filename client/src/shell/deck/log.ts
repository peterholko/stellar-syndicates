// §log (Oct 28 2026, Peter: "the Log panel needs a complete overhaul"): two
// tabs over the shared models. Inbox is what needs you (core/derive/inbox.ts),
// grouped, one action a row, dismiss and snooze kept per corporation. History
// is what happened (core/derive/history.ts): one row per event or episode,
// filters, day headers, unread marks and a report behind every row. Battles
// live under History's Combat filter; the old toast mirror is gone, because
// everything it held is a History row now.

import { archiveRows } from "../../core/derive/battlearchive";
import { COMMODITY_VALUE } from "../../core/derive/fleet";
import { ageLabel, fmtBuildDur, gameClock } from "../../core/derive/format";
import { commandDelayTo, systemName } from "../../core/derive/geo";
import {
  dayLabel, goodsValue, HISTORY_FILTERS, historyRows, rowActions, unreadNewsCount, type HistoryFilter, type HistoryRow,
} from "../../core/derive/history";
import {
  computeInbox, INBOX_GROUPS, loadInboxPrefs, saveInboxPrefs, SNOOZE_S, visibleInbox,
  type Go, type InboxItem, type InboxPrefs,
} from "../../core/derive/inbox";
import { nextDecisionLabel } from "../../core/derive/orders";
import { freshNews, latestNews, newsRows, seasonLine } from "../../core/derive/season";
import type { CoreEvent } from "../../core/events";
import { icon, label } from "../../icons";
import type { Commodity, EntityId } from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

export type DeckInboxItem = InboxItem;

interface DeckLogHooks {
  go(route: DeckRoute): void;
  focusSystem(id: string): void;
  focusFleet(id: string): void;
  notice?(text: string): void;
}

type LogTab = "inbox" | "history" | "news";

/** Inbox rows shown before "+N more". */
const INBOX_SHOWN = 5;

export class DeckLogRoutes {
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
  /** History's unread boundary while the tab is open, so new rows stay marked. */
  private readAtOpen: number | null = null;
  /** §season: the News tab's, likewise. */
  private newsAtOpen: number | null = null;
  private opened = false;
  private appliedRoute: DeckRoute | null = null;
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: DeckLogHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "log") {
      this.readAtOpen = null;
      this.newsAtOpen = null;
      this.seenAtOpen = null;
      this.opened = false;
      return false;
    }
    if (!this.opened) {
      // A fresh open shows what needs you, else what happened.
      this.opened = true;
      this.tab = this.items().length ? "inbox" : "history";
    }
    // A route's query applies once, when it is navigated to.
    if (route !== this.appliedRoute) {
      this.appliedRoute = route;
      this.applyQuery(route);
    }
    const prefs = this.loadPrefs();
    this.markTabRead();
    const signature = sheetFingerprint([
      this.tab, this.filter, this.system, this.showAll, this.awayClosed, [...this.open].sort(), prefs, this.ctx.state.season,
      Math.floor(this.ctx.state.simTime / 5), this.ctx.state.systems, this.ctx.state.ghosts, this.ctx.state.battles,
      this.ctx.state.battleReports, this.ctx.state.captureReports, this.ctx.state.battleRecords.map((record) => [record.id, record.outcome]),
      this.ctx.state.standingOrders, this.ctx.state.timeline, this.ctx.state.operations, this.ctx.state.welcome,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, this.pageHtml());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "log") return false;
    const { deckAct, key = "", tab, filter } = button.dataset;
    switch (deckAct) {
      case "log-tab":
        this.tab = tab === "history" || tab === "battles" ? "history" : tab === "news" ? "news" : "inbox";
        if (tab === "battles") this.filter = "combat";
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
        const action = button.dataset.source === "row"
          ? this.historyAt(key) && rowActions(this.historyAt(key)!, systemName)[index]
          : this.items().find((item) => item.key === key)?.actions[index];
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
      case "log-battle-open": {
        // The same route a battle marker on the map opens.
        const id = button.dataset.battle ?? "";
        const running = this.ctx.state.battles.some((battle) => battle.id === id);
        this.hooks.go({ name: "battle", params: { id, label: running ? "Ongoing battle" : "Battle replay" } });
        return true;
      }
      default:
        return false;
    }
    this.invalidate();
    this.render(route, true);
    return true;
  }

  onCore(events: CoreEvent[], route: DeckRoute | null): void {
    if (events.some((event) => event.kind === "TimelineApplied") || route?.name === "log") this.invalidate();
  }

  /** The visible inbox; Command's digest shows its top four. */
  items(): DeckInboxItem[] {
    return visibleInbox(computeInbox(this.ctx.state), this.loadPrefs(), liveSimTime(), this.ctx.state.systems.length > 0);
  }

  runPrimary(key: string): void {
    const action = this.items().find((item) => item.key === key)?.actions[0];
    if (action) this.run(action.go);
    this.invalidate();
  }

  decisionKeys(): string[] {
    return this.items().map((item) => item.key);
  }

  /** New inbox items, plus History rows of warn or worse not yet seen. */
  badgeCount(route: DeckRoute | null): number {
    if (route?.name === "log") this.markTabRead();
    const seen = this.loadPrefs().seen;
    const unreadDecisions = this.decisionKeys().filter((key) => !seen.includes(key)).length;
    return unreadDecisions + unreadNewsCount(this.ctx.state.timeline, this.loadPrefs().readThrough ?? this.ctx.state.awaySince);
  }

  invalidate(): void {
    this.signature = "";
  }

  private applyQuery(route: DeckRoute): void {
    const { tab, filter, system } = route.query ?? {};
    if (tab === "inbox" || tab === "log") this.tab = "inbox";
    if (tab === "history") this.tab = "history";
    if (tab === "news") this.tab = "news";
    if (tab === "battles") {
      this.tab = "history";
      this.filter = "combat";
    }
    if (filter && HISTORY_FILTERS.some((entry) => entry.key === filter)) this.filter = filter as HistoryFilter;
    if (system) this.system = system;
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
    const keys = this.decisionKeys();
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
    const systemLabel = (id: string) => state.galaxy?.systems.find((entry) => entry.id === id)?.name ?? "System";
    switch (go.to) {
      case "system": this.hooks.focusSystem(go.id); break;
      case "fleet": this.hooks.focusFleet(go.id); break;
      case "withdraw": this.ctx.intent.beginFleetCommand({ type: "Withdraw", fleet_id: go.fleet }); break;
      case "send":
        this.ctx.send(go.command);
        if (go.done) this.hooks.notice?.(go.done);
        break;
      case "history":
        this.tab = "history";
        this.filter = go.filter ?? "all";
        this.system = go.system ?? null;
        break;
      case "showAll": this.showAll = true; break;
      case "panel": {
        const params = go.params ?? {};
        if (go.panel === "logistics") this.hooks.go({ name: "logistics", query: go.query ?? {} });
        else if (go.panel === "battle") this.hooks.go({ name: "battle", params: { label: "Battle", ...params } });
        else if (go.panel === "production") this.hooks.go({ name: "system", params: { id: params.id, systemLabel: systemLabel(params.id) }, query: { tab: "production" } });
        else if (go.panel === "build") this.hooks.go({ name: "build", params: { systemId: params.id, systemLabel: systemLabel(params.id) } });
        else if (go.panel === "world") this.hooks.go({ name: "world", params: { ...params, systemLabel: systemLabel(params.systemId) } });
        else this.hooks.go({ name: go.panel });
        break;
      }
    }
  }

  // ── Page ──

  private pageHtml(): string {
    const inbox = this.items();
    const rows = this.history();
    const unread = rows.filter((row) => row.unread).length;
    const news = freshNews(this.ctx.state.season, this.loadPrefs().newsRead ?? latestNews(this.ctx.state.season));
    const tabs = `<nav class="deck-tabs deck-log__tabs" aria-label="Log">`
      + `<button type="button" data-deck-act="log-tab" data-tab="inbox" aria-selected="${this.tab === "inbox"}">Inbox<em>${inbox.length || ""}</em></button>`
      + `<button type="button" data-deck-act="log-tab" data-tab="history" aria-selected="${this.tab === "history"}">History<em>${unread ? `${unread} new` : ""}</em></button>`
      + `<button type="button" data-deck-act="log-tab" data-tab="news" aria-selected="${this.tab === "news"}">News<em>${news ? `${news} new` : ""}</em></button></nav>`;
    const body = this.tab === "inbox" ? this.inboxHtml(inbox) : this.tab === "news" ? this.newsHtml() : this.historyHtml(rows);
    return `<section class="deck-page deck-log">${tabs}${body}</section>`;
  }

  /** §season: the Galactic News Network, newest first. */
  private newsHtml(): string {
    const season = this.ctx.state.season;
    const line = seasonLine(season);
    const head = line ? `<p class="deck-news-head">${esc(line)}</p>` : "";
    const rows = newsRows(season, liveSimTime(), this.newsAtOpen);
    if (!rows.length) return `${head}<div class="deck-muted">No news has reached you yet.</div>`;
    return `${head}<div class="deck-log__list">${rows.map((row) => {
      const go = row.fleet ? ` data-fleet="${esc(row.fleet)}"` : row.system ? ` data-system="${esc(row.system)}"` : "";
      const button = go ? `<button type="button" class="deck-logrow__act" data-deck-act="log-news-go"${go}>View</button>` : "";
      return `<article class="deck-logrow deck-newsrow is-${row.tone}${row.unread ? " is-unread" : ""}${row.yours ? " is-yours" : ""}${row.free ? " is-free" : ""}">`
        + `<i class="deck-logrow__bar"></i><img class="deck-newsrow__source" src="${esc(row.emblem)}" alt="${esc(row.source)}" title="${esc(row.source)}" decoding="async">`
        + `<span class="deck-logrow__text"><b>${esc(row.headline)}</b><small>${esc(row.meta)}</small></span>${button}</article>`;
    }).join("")}</div>`;
  }

  private inboxHtml(items: InboxItem[]): string {
    if (!items.length) return `<div class="deck-log-clear">${icon("success", "sm")}<span>All clear · ${esc(nextDecisionLabel())}</span></div>`;
    const shown = this.showAll ? items : items.slice(0, INBOX_SHOWN);
    const groups = INBOX_GROUPS.map((group) => {
      const mine = shown.filter((item) => item.group === group.key);
      return mine.length ? `<h4 class="deck-log__group"><span>${group.label}</span><b>${items.filter((item) => item.group === group.key).length}</b></h4>${mine.map((item) => this.inboxRowHtml(item)).join("")}` : "";
    }).join("");
    const more = items.length > INBOX_SHOWN
      ? `<button type="button" class="deck-log__more" data-deck-act="log-more">${this.showAll ? "Show fewer" : `+${items.length - INBOX_SHOWN} more`}</button>` : "";
    return `<div class="deck-log__list">${groups}${more}</div>`;
  }

  private inboxRowHtml(item: InboxItem): string {
    const [primary, ...rest] = item.actions;
    const delay = primary?.go.to === "withdraw" ? commandDelayTo(primary.go.pos) : null;
    const button = primary
      ? `<button type="button" class="deck-logrow__act${primary.danger ? " is-danger" : ""}" data-deck-act="log-go" data-source="inbox" data-key="${esc(item.key)}" data-index="0"${delay !== null ? ` title="Order arrives in ~${esc(fmtBuildDur(delay))}"` : ""}>${esc(primary.label)}</button>`
      : "";
    const menu = `<details class="deck-logrow__menu"><summary aria-label="More actions">⋯</summary><div>`
      + rest.map((action, index) => `<button type="button" data-deck-act="log-go" data-source="inbox" data-key="${esc(item.key)}" data-index="${index + 1}">${esc(action.label)}</button>`).join("")
      + `<button type="button" data-deck-act="log-snooze" data-key="${esc(item.key)}">Snooze 1h</button>`
      + `<button type="button" data-deck-act="log-dismiss" data-key="${esc(item.key)}">Dismiss</button></div></details>`;
    const fresh = this.seenAtOpen && !this.seenAtOpen.has(item.key) ? " is-unread" : "";
    return `<article class="deck-logrow is-${item.tone}${fresh}" title="${esc(item.hint)}"><i class="deck-logrow__bar"></i>`
      + `<span class="deck-logrow__icon">${icon(item.icon, "sm")}</span><span class="deck-logrow__text"><b>${esc(item.title)}</b>${item.facts ? `<small>${esc(item.facts)}</small>` : ""}</span>`
      + `${button}${menu}</article>`;
  }

  private history(): HistoryRow[] {
    const prefs = this.loadPrefs();
    return historyRows(this.ctx.state.timeline, {
      filter: this.filter, system: this.system, now: liveSimTime(),
      readThrough: this.readAtOpen ?? prefs.readThrough ?? this.ctx.state.awaySince,
      nameOf: systemName, valueOf: (good: Commodity) => COMMODITY_VALUE[good] ?? 0,
    });
  }

  private historyAt(key: string): HistoryRow | undefined {
    return this.history().find((row) => row.key === key);
  }

  private historyHtml(rows: HistoryRow[]): string {
    const now = liveSimTime();
    const filters = `<div class="deck-log__filters" role="toolbar" aria-label="Filter history">${HISTORY_FILTERS.map((entry) =>
      `<button type="button" class="deck-chip${entry.key === this.filter ? " is-on" : ""}" data-deck-act="log-filter" data-filter="${entry.key}" aria-pressed="${entry.key === this.filter}">${entry.label}</button>`).join("")}`
      + (this.system ? `<button type="button" class="deck-chip is-on" data-deck-act="log-system-clear" aria-label="Clear the system filter">${esc(systemName(this.system))} ✕</button>` : "") + `</div>`;
    let day = "";
    const body = rows.map((row) => {
      const header = dayLabel(row.at, now);
      const heading = header !== day ? `<h4 class="deck-log__day">${esc(header)}</h4>` : "";
      day = header;
      return heading + this.historyRowHtml(row, now);
    }).join("");
    const engagements = this.filter === "combat" ? this.engagementsHtml() : "";
    const empty = `<div class="deck-log-clear"><span>${this.filter === "all" && !this.system ? "Nothing yet." : "Nothing here yet."}</span></div>`;
    return `${this.awayHtml()}${filters}${engagements}<div class="deck-log__list">${body || empty}</div>`;
  }

  private historyRowHtml(row: HistoryRow, now: number): string {
    const open = this.open.has(row.key);
    return `<article class="deck-logrow is-${row.severity}${row.unread ? " is-unread" : ""}${open ? " is-open" : ""}">`
      + `<button type="button" class="deck-logrow__main" data-deck-act="log-row" data-key="${esc(row.key)}" aria-expanded="${open}">`
      + `<i class="deck-logrow__bar"></i><span class="deck-logrow__icon">${icon(row.icon, "sm")}</span>`
      + `<span class="deck-logrow__text"><b>${esc(row.title)}</b>${row.detail ? `<small>${esc(row.detail)}</small>` : ""}</span>`
      + `<time title="${esc(gameClock(row.at))}">${esc(ageLabel(now - row.at))}</time></button>`
      + (open ? this.reportHtml(row) : "") + `</article>`;
  }

  /** The row's report: its lines, its goods priced, and its ways out. */
  private reportHtml(row: HistoryRow): string {
    const valueOf = (good: Commodity) => COMMODITY_VALUE[good] ?? 0;
    const lines = row.entries.length > 1 || row.title !== row.entries[0].text
      ? `<ol class="deck-logreport__lines">${row.entries.map((entry) => `<li><time>${esc(gameClock(entry.at_time))}</time><span>${esc(entry.text)}</span></li>`).join("")}</ol>` : "";
    const duration = row.entries.map((entry) => entry.data?.duration).find((value) => value != null);
    const ships = row.entries.map((entry) => entry.data?.ships).find((value) => value != null);
    const facts = [ships ? `${ships} raider${ships === 1 ? "" : "s"}` : "", duration ? `held ${fmtBuildDur(duration)}` : ""].filter(Boolean).join(" · ");
    const goods = row.goods.length
      ? `<table class="deck-logreport__goods"><thead><tr><th>${row.goods.some(([, n]) => n < 0) ? "Lost" : "Goods"}</th><th>Units</th><th>Value</th></tr></thead><tbody>`
        + row.goods.map(([good, n]) => `<tr><td>${esc(label(good))}</td><td class="is-num${n < 0 ? " is-neg" : ""}">${n > 0 ? "+" : "−"}${Math.abs(n).toLocaleString()}</td><td class="is-num">${Math.round(Math.abs(n) * valueOf(good)).toLocaleString()} Cr</td></tr>`).join("")
        + `</tbody><tfoot><tr><td>Total</td><td class="is-num">${row.goods.reduce((sum, [, n]) => sum + Math.abs(n), 0).toLocaleString()}</td><td class="is-num">${Math.round(goodsValue(row.goods, valueOf)).toLocaleString()} Cr</td></tr></tfoot></table>`
      : "";
    const actions = rowActions(row, systemName);
    const buttons = actions.length
      ? `<div class="deck-logreport__acts">${actions.map((action, index) => `<button type="button"${action.primary ? ` class="is-primary"` : ""} data-deck-act="log-go" data-source="row" data-key="${esc(row.key)}" data-index="${index}">${esc(action.label)}</button>`).join("")}</div>` : "";
    return `<div class="deck-logreport">${facts ? `<p class="deck-logreport__facts">${esc(facts)}</p>` : ""}${lines}${goods}${buttons}</div>`;
  }

  /** §welcome (R9): the server's report on return, as chips over History. */
  private awayHtml(): string {
    const welcome = this.ctx.state.welcome;
    if (!welcome || this.awayClosed || !welcome.happened.length) return "";
    const chips = welcome.happened.slice(0, 4).map((line) => `<span class="deck-chip is-${line.severity}" title="${esc(line.text)}">${esc(line.text)}</span>`).join("");
    return `<div class="deck-log__away"><header><b>While you were away · ${esc(fmtBuildDur(welcome.away))}</b>`
      + `<button type="button" data-deck-act="log-away-close" aria-label="Close the summary">✕</button></header><div class="deck-log__chips">${chips}</div></div>`;
  }

  /** §battle-archive: every battle record still served, newest first. */
  private engagementsHtml(): string {
    const rows = archiveRows(this.ctx.state, liveSimTime());
    if (!rows.length) return "";
    return `<h4 class="deck-log__group"><span>Engagements</span><b>${rows.length}</b></h4><div class="deck-battle-list">${rows.map((row) =>
      `<div class="deck-battle-row is-${row.tone}"><span><b>${esc(row.where)}</b><small>${esc(row.ago)} · ${esc(row.sides)}</small></span>`
      + `<em>${esc(row.outcome)}</em><button type="button" data-deck-act="log-battle-open" data-battle="${esc(row.id)}" title="${row.running ? "Follow the battle" : "Replay"}">Open</button></div>`).join("")}</div>`;
  }
}

function esc(value: string): string {
  return value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}


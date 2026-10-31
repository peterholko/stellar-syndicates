// §log (Oct 28 2026, Peter: "the Log panel needs a complete overhaul"): the
// History tab's rows, built from the server's structured journal (protocol
// v71). One row per event or episode: a raid's inbound, raiding and ended lines
// become one row showing its latest state; like lines that land together
// collapse to one. Every row keeps what it names (links) and its numbers (the
// report). Pure: names and prices come in as options, so both shells and the
// tests share it.

import type { IconKey } from "../../icons";
import type { Commodity, EntityId, LogKind, LogPhase, LogRefs, TimelineEntry, TimelineSeverity } from "../../protocol";
import { label } from "../../icons";
import type { Go } from "./inbox";

export type HistoryFilter = "all" | "raids" | "combat" | "trade" | "build" | "research" | "explore" | "ops" | "fleet" | "diplomacy";

export const HISTORY_FILTERS: readonly { key: HistoryFilter; label: string; kinds: readonly LogKind[] }[] = [
  { key: "all", label: "All", kinds: [] },
  { key: "raids", label: "Raids", kinds: ["raid", "vendetta"] },
  { key: "combat", label: "Combat", kinds: ["battle", "blockade", "capture"] },
  { key: "trade", label: "Trade", kinds: ["trade", "freight", "trader"] },
  { key: "build", label: "Build", kinds: ["build", "production"] },
  { key: "research", label: "Research", kinds: ["research"] },
  { key: "explore", label: "Explore", kinds: ["survey", "signal", "territory"] },
  { key: "ops", label: "Ops", kinds: ["operation", "charter", "medal"] },
  { key: "fleet", label: "Fleet", kinds: ["fleet", "refusal"] },
  { key: "diplomacy", label: "Diplomacy", kinds: ["diplomacy", "syndicate", "admin"] },
];

export const KIND_ICON: Record<LogKind, IconKey> = {
  raid: "raid", vendetta: "raider", blockade: "blockade", battle: "battle", capture: "captured",
  trade: "market", freight: "freightRoute", trader: "free_trader", build: "build", production: "refinery",
  research: "structureAcademy", survey: "sensor", signal: "intel", territory: "claim", operation: "manifest",
  diplomacy: "ally", syndicate: "syndicate", fleet: "fleet", refusal: "warning", charter: "commandCenter",
  medal: "success", admin: "info",
};

const TERMINAL: readonly LogPhase[] = ["ended", "lifted", "completed", "failed", "expired", "done"];
/** Like lines further apart than this stay separate rows. */
const COLLAPSE_S = 15 * 60;

/** Entries saved before v71 carry no kind; their prose still says what they are. */
const LEGACY: readonly [RegExp, LogKind][] = [
  [/^Vendetta · /, "vendetta"],
  [/operation|^Contract board/i, "operation"],
  [/^Pirate|Pirates|privateer|^Raided /, "raid"],
  [/blockade|BLOCKADE|STRIPPED|^Prize taken|CONTESTED/, "blockade"],
  [/CAPTURED|^You LOST|landing|LANDING|claimed/, "capture"],
  [/^Order refused|^Can't |refused/i, "refusal"],
  [/^Construction|developed —|^Launched|complete\.$|^Refit/, "build"],
  [/SUSPENDED|producing again|^Brownout|^Power restored|RATIONING|[Ff]ood|PROVISIONS|WELL SUPPLIED/, "production"],
  [/^Research|^Tier \d|^Research grant|^Designation/, "research"],
  [/^Survey|^Prospected|^Scout report|TRAIT|pocket/, "survey"],
  [/[Ss]ignal|^First find|^Report in/, "signal"],
  [/^Free trader|free trader| left .*\.$/, "trader"],
  [/^Sold|^Limit|^Booked|freight|^Delivery arrived|auto-shipped|^Supply to|^Storage full/i, "trade"],
  [/^Charter milestone|Authority/, "charter"],
  [/medal/i, "medal"],
  [/[Ss]yndicate|rung/, "syndicate"],
  [/[Dd]iplomatic|WAR |agreement|reprisal|Separation/, "diplomacy"],
];

export function kindOf(entry: TimelineEntry): LogKind {
  return entry.kind ?? LEGACY.find(([re]) => re.test(entry.text))?.[1] ?? "fleet";
}

export interface HistoryRow {
  key: string;
  kind: LogKind;
  icon: IconKey;
  severity: TimelineSeverity;
  /** One line: what happened, with its place. */
  title: string;
  /** The numbers: goods, raiders, an ETA, how many collapsed. */
  detail: string;
  /** The latest entry's time. */
  at: number;
  /** Oldest first. */
  entries: TimelineEntry[];
  group: string | null;
  phase: LogPhase | null;
  refs: LogRefs;
  /** Net goods for this player, most valuable first. */
  goods: [Commodity, number][];
  unread: boolean;
}

export interface HistoryOptions {
  filter?: HistoryFilter;
  /** Only rows that name this system. */
  system?: EntityId | null;
  /** Rows newer than this sim-time are unread. */
  readThrough: number;
  now: number;
  nameOf(system: EntityId): string;
  valueOf(good: Commodity): number;
}

const terminal = (phase: LogPhase | null | undefined): boolean => !!phase && TERMINAL.includes(phase);
const shape = (text: string): string => text.replace(/\d[\d,.]*/g, "#");

/** History rows, newest first. */
export function historyRows(entries: readonly TimelineEntry[], options: HistoryOptions): HistoryRow[] {
  const kinds = HISTORY_FILTERS.find((filter) => filter.key === (options.filter ?? "all"))?.kinds ?? [];
  const rows: HistoryRow[] = [];
  const open = new Map<string, HistoryRow>();
  for (const entry of [...entries].sort((a, b) => a.at_time - b.at_time || (a.id ?? 0) - (b.id ?? 0))) {
    const kind = kindOf(entry);
    if (entry.group) {
      const row = open.get(entry.group);
      if (row) absorb(row, entry);
      else {
        const fresh = startRow(entry, kind);
        rows.push(fresh);
        open.set(entry.group, fresh);
      }
      if (terminal(entry.phase)) open.delete(entry.group);
      continue;
    }
    const last = rows.at(-1);
    if (last && !last.group && last.kind === kind && last.severity === entry.severity
      && shape(last.entries.at(-1)!.text) === shape(entry.text) && entry.at_time - last.at <= COLLAPSE_S) {
      absorb(last, entry);
      continue;
    }
    rows.push(startRow(entry, kind));
  }
  return rows
    .filter((row) => !kinds.length || kinds.includes(row.kind))
    .filter((row) => !options.system || row.refs.system === options.system)
    .map((row) => describe(row, options))
    .reverse();
}

function startRow(entry: TimelineEntry, kind: LogKind): HistoryRow {
  return { key: `${entry.id ?? 0}:${entry.at_time}:${entry.text.slice(0, 24)}`, kind, icon: KIND_ICON[kind],
    severity: entry.severity, title: entry.text, detail: "", at: entry.at_time, entries: [entry],
    group: entry.group ?? null, phase: entry.phase ?? null, refs: { ...entry.refs }, goods: [], unread: false };
}

function absorb(row: HistoryRow, entry: TimelineEntry): void {
  row.entries.push(entry);
  row.at = entry.at_time;
  row.severity = entry.severity;
  row.phase = entry.phase ?? row.phase;
  row.refs = { ...entry.refs, ...row.refs };
}

function describe(row: HistoryRow, options: HistoryOptions): HistoryRow {
  const totals = new Map<Commodity, number>();
  for (const entry of row.entries) {
    for (const [good, n] of Object.entries(entry.data?.goods ?? {}) as [Commodity, number][]) {
      totals.set(good, (totals.get(good) ?? 0) + n);
    }
  }
  row.goods = [...totals].filter(([, n]) => n !== 0)
    .sort((a, b) => Math.abs(b[1]) * options.valueOf(b[0]) - Math.abs(a[1]) * options.valueOf(a[0]));
  row.unread = row.entries.some((entry) => entry.at_time > options.readThrough);
  const latest = row.entries.at(-1)!;
  const place = row.refs.system ? options.nameOf(row.refs.system) : "";
  const goods = goodsLine(row.goods);
  row.title = latest.text;
  row.detail = goods;
  if (row.kind === "raid" && row.group) {
    const inbound = row.entries.find((entry) => entry.phase === "inbound")?.data;
    if (row.phase === "inbound") {
      const eta = inbound?.eta ? inbound.eta - options.now : null;
      row.title = `Raid inbound · ${place}`;
      row.detail = [inbound?.ships ? `${inbound.ships} raider${inbound.ships === 1 ? "" : "s"}` : "",
        eta === null ? "" : eta > 30 ? `ETA ~${Math.ceil(eta / 60)}m` : "arriving"].filter(Boolean).join(" · ");
    } else if (row.phase === "attacking") {
      row.title = `Pirates attacking · ${place}`;
      row.detail = "your defenses are fighting them";
    } else if (row.phase === "raiding") {
      row.title = `Pirate raid · ${place}`;
      row.detail = "stripping its stores";
    } else if (row.phase === "ended") {
      row.title = row.goods.length ? `Raided · ${place}` : `Raid on ${place} · nothing taken`;
    }
  } else if (row.kind === "blockade" && row.group) {
    row.title = row.phase === "contested" ? `${place} contested` : row.phase === "lifted" ? `Blockade lifted · ${place}` : `Blockaded · ${place}`;
  } else if (row.entries.length > 1 && !row.group) {
    row.detail = [`×${row.entries.length}`, goods].filter(Boolean).join(" · ");
  }
  return row;
}

/** §log: how a home raid's journal news toasts, by phase: the warning with
 * its raiders and ETA, the start, and the end with what was taken. */
export function logToast(entry: TimelineEntry, nameOf: (system: EntityId) => string, now: number):
  { title: string; message: string; tone: "bad" | "warn" | "good" } | null
{
  if (entry.kind !== "raid" || !entry.group) return null;
  const place = entry.refs?.system ? nameOf(entry.refs.system) : "home";
  const taken = (Object.entries(entry.data?.goods ?? {}) as [Commodity, number][]).filter(([, n]) => n < 0);
  switch (entry.phase) {
    case "inbound": {
      const ships = entry.data?.ships, eta = entry.data?.eta ? entry.data.eta - now : null;
      return { title: `Raid inbound · ${place}`, tone: "bad", message: [ships ? `${ships} raider${ships === 1 ? "" : "s"}` : "",
        eta === null ? "" : eta > 30 ? `ETA ~${Math.ceil(eta / 60)}m` : "arriving"].filter(Boolean).join(" · ") || "Pirates are on their way." };
    }
    case "attacking": return { title: `Raid · ${place}`, message: "Your defenses are fighting them.", tone: "warn" };
    case "raiding": return { title: `Raid · ${place}`, message: "Pirates are stripping its stores.", tone: "bad" };
    case "ended": return taken.length
      ? { title: `Raided · ${place}`, message: `Took ${goodsLine(taken, false)}`, tone: "bad" }
      : { title: `Raid · ${place}`, message: "Nothing taken.", tone: "good" };
    default: return null;
  }
}

/** Unread news of warn or worse since `readThrough`, one per episode: the
 * Log's badge, alongside its new inbox rows. */
export function unreadNewsCount(entries: readonly TimelineEntry[], readThrough: number): number {
  const episodes = new Set<string>();
  let n = 0;
  for (const entry of entries) {
    if (entry.at_time <= readThrough || (entry.severity !== "bad" && entry.severity !== "warn")) continue;
    if (entry.group) {
      if (episodes.has(entry.group)) continue;
      episodes.add(entry.group);
    }
    n++;
  }
  return n;
}

/** "−46 Ferrite Ore · −14 Provisions"; unsigned, "46 Ferrite Ore · 14 Provisions". */
export function goodsLine(goods: readonly [Commodity, number][], signed = true): string {
  return goods.map(([good, n]) => `${signed ? n > 0 ? "+" : "−" : ""}${Math.abs(n).toLocaleString()} ${label(good)}`).join(" · ");
}

/** What a row's goods were worth at base prices: "~494 Cr". */
export function goodsValue(goods: readonly [Commodity, number][], valueOf: (good: Commodity) => number): number {
  return goods.reduce((sum, [good, n]) => sum + Math.abs(n) * valueOf(good), 0);
}

/** A day header for the game clock's days: "Today · D6", "Yesterday · D5", "D3". */
export function dayLabel(at: number, now: number): string {
  const day = Math.floor(Math.max(0, at) / 86_400) + 1;
  const today = Math.floor(Math.max(0, now) / 86_400) + 1;
  return day === today ? `Today · D${day}` : day === today - 1 ? `Yesterday · D${day}` : `D${day}`;
}

/** The ways out of a row: its places, fleets, operations and battles. */
export function rowActions(row: HistoryRow, nameOf: (system: EntityId) => string): { label: string; go: Go; primary?: boolean }[] {
  const out: { label: string; go: Go; primary?: boolean }[] = [];
  const { system, fleet, operation, battle } = row.refs;
  if (battle) out.push({ label: "Replay", go: { to: "panel", panel: "battle", params: { id: battle } }, primary: true });
  if (row.kind === "raid" && system) out.push({ label: `${row.phase === "ended" ? "Show" : "Defend"} ${nameOf(system)}`, go: { to: "system", id: system }, primary: !out.length });
  if (row.kind === "raid" && fleet && row.phase === "ended" && row.goods.length) out.push({ label: "Track the haul", go: { to: "fleet", id: fleet } });
  else if (fleet && row.kind !== "raid") out.push({ label: "Open fleet", go: { to: "fleet", id: fleet }, primary: !out.length });
  if (operation) out.push({ label: "Open operation", go: { to: "panel", panel: "operations" }, primary: !out.length });
  if (system && row.kind !== "raid") out.push({ label: `Show ${nameOf(system)}`, go: { to: "system", id: system }, primary: !out.length });
  if (system) out.push({ label: `History of ${nameOf(system)}`, go: { to: "history", system } });
  return out;
}

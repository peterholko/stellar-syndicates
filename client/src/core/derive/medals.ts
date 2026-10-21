// §medals (Oct 19 2026, Peter): weekly medals and the corporation card. Each
// week the top three corporations in each category win gold, silver and bronze
// on the week's own gains (crates/sim/src/rankings.rs). Everything here reads
// the published, light-delayed rankings table, the same for every corporation.
import type { RankingRow } from "../../protocol";
import { fmt } from "./format";

/** A medal week in sim seconds (rankings::MEDAL_WEEK_S). */
export const MEDAL_WEEK_S = 7 * 24 * 60 * 60;

export interface RankingCategory {
  /** The server's slug (rankings::RankingCategory::slug). */
  slug: string;
  label: string;
  /** Whether the category awards weekly medals (all but Recovery). */
  medal: boolean;
  value(row: RankingRow): number;
  text(row: RankingRow): string;
}

export const RANKING_CATEGORIES: RankingCategory[] = [
  { slug: "valuation", label: "Valuation", medal: true, value: (r) => r.valuation, text: (r) => `${fmt(r.valuation)} Cr` },
  { slug: "trade_throughput", label: "Trade throughput", medal: true, value: (r) => r.trade_throughput, text: (r) => fmt(r.trade_throughput) },
  { slug: "market_profit", label: "Net market profit", medal: true, value: (r) => r.market_profit, text: (r) => `${fmt(r.market_profit)} Cr` },
  { slug: "cargo_captured", label: "Cargo captured", medal: true, value: (r) => r.cargo_captured, text: (r) => fmt(r.cargo_captured) },
  { slug: "cargo_protected", label: "Cargo protected", medal: true, value: (r) => r.cargo_protected, text: (r) => fmt(r.cargo_protected) },
  { slug: "battle_efficiency", label: "Battle efficiency", medal: true, value: (r) => r.battle_ranked ? r.battle_efficiency : -Infinity,
    text: (r) => r.battle_ranked ? `×${r.battle_efficiency.toFixed(2)}` : "provisional" },
  { slug: "systems_developed", label: "Systems developed", medal: true, value: (r) => r.systems_developed, text: (r) => fmt(r.systems_developed) },
  { slug: "intel_gathered", label: "Intel gathered", medal: true, value: (r) => r.intel_gathered, text: (r) => fmt(r.intel_gathered) },
  { slug: "recovery", label: "Recovery", medal: false, value: (r) => r.recovery, text: (r) => `${fmt(r.recovery)} Cr` },
];

/** A row's gold, silver and bronze across every category. */
export function medalTotals(row: RankingRow): [number, number, number] {
  return (row.medals ?? []).reduce<[number, number, number]>(
    ([g, s, b], t) => [g + t.gold, s + t.silver, b + t.bronze], [0, 0, 0]);
}

/** The week under way (1 = the galaxy's first) and the time until its medals. */
export function medalWeek(simTime: number): { week: number; left: number } {
  const t = Math.max(0, simTime);
  return { week: Math.floor(t / MEDAL_WEEK_S) + 1, left: MEDAL_WEEK_S - (t % MEDAL_WEEK_S) };
}

/** "4d 6h", "6h 20m", "12m". */
export function fmtDays(secs: number): string {
  const m = Math.max(0, Math.floor(secs / 60)), h = Math.floor(m / 60), d = Math.floor(h / 24);
  return d > 0 ? `${d}d ${h % 24}h` : h > 0 ? `${h}h ${m % 60}m` : `${m}m`;
}

export interface CorpCard {
  row: RankingRow;
  me: boolean;
  /** The categories it has medals in, in display order. */
  medals: { slug: string; label: string; gold: number; silver: number; bronze: number }[];
  /** Its place in each category of the published table (null: nothing to
   * rank, a value of zero or a provisional one; the titles' rule). */
  standing: { label: string; rank: number | null; of: number; text: string }[];
  week: number;
  left: number;
}

/** One corporation's card, from the published table. Null if it isn't in it. */
export function corpCard(rows: readonly RankingRow[], id: string, me: string | null, simTime: number): CorpCard | null {
  const row = rows.find((r) => String(r.player_id) === id);
  if (!row) return null;
  const medals = RANKING_CATEGORIES.filter((c) => c.medal).map((c) => {
    const t = row.medals?.find((m) => m.category === c.slug);
    return { slug: c.slug, label: c.label, gold: t?.gold ?? 0, silver: t?.silver ?? 0, bronze: t?.bronze ?? 0 };
  }).filter((m) => m.gold + m.silver + m.bronze > 0);
  const standing = RANKING_CATEGORIES.map((c) => {
    const ranked = rows.filter((r) => Number.isFinite(c.value(r)) && c.value(r) > 0)
      .sort((a, b) => c.value(b) - c.value(a) || String(a.player_id).localeCompare(String(b.player_id), undefined, { numeric: true }));
    const at = ranked.indexOf(row);
    return { label: c.label, rank: at >= 0 ? at + 1 : null, of: rows.length, text: c.text(row) };
  });
  return { row, me: me !== null && String(row.player_id) === String(me), medals, standing, ...medalWeek(simTime) };
}

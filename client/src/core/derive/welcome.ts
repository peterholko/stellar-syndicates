import { label } from "../../icons";
import type { SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";
import { foundingPlan, type FoundingStep } from "./founding";

/** Shorter absences are a tab switch, not a return. */
export const WELCOME_MIN_AWAY_S = 10 * 60;

export interface WelcomeReport {
  /** Sim seconds since the server last saw this corporation online. */
  away: number;
  /** Receipts and news that arrived while away, newest facts first. */
  happened: string[];
  /** Work still in flight, from the founding guide when there is one. */
  underway: string[];
  next: FoundingStep | null;
  /** The first receipt page did not reach back to the departure. */
  partial: boolean;
}

type Tally = Map<string, { units: number; credits: number }>;

function add(tally: Tally, item: string, units: number, credits: number): void {
  const row = tally.get(item) ?? { units: 0, credits: 0 };
  row.units += units;
  row.credits += credits;
  tally.set(item, row);
}

function tallyLine(verb: string, tally: Tally): string | null {
  if (!tally.size) return null;
  const total = [...tally.values()].reduce((sum, row) => sum + row.credits, 0);
  const items = [...tally].map(([item, row]) => `${row.units.toLocaleString()} ${label(item)}`).join(", ");
  return `${verb} ${items} for ${Math.round(total).toLocaleString()} Cr`;
}

/** §founding R11: the charter report on return. Only arrived receipts and
 * arrived timeline light; a sale still in flight is not reported as done. */
export function welcomeReport(st: ViewState, home?: SystemStateView): WelcomeReport | null {
  if (!st.awaySet || st.playerId === null || st.awaySince <= 0) return null;
  const since = st.awaySince;
  const away = st.simTime - since;
  if (away < WELCOME_MIN_AWAY_S) return null;

  const sold: Tally = new Map();
  const bought: Tally = new Map();
  for (const entry of st.transactions.entries) {
    if (entry.reported_at <= since) continue;
    const d = entry.details;
    if (d.kind === "trade") {
      const t = d.trade;
      if (t.event === "Sold" || (t.event === "LimitFilled" && t.side === "sell")) add(sold, t.commodity, t.units, t.units * t.unit_price);
      else if (t.event === "Bought" || (t.event === "LimitFilled" && t.side === "buy")) add(bought, t.commodity, t.units, t.units * t.unit_price);
    } else if (d.kind === "sale") add(sold, d.item, d.units, d.units * d.unit_price);
    else if (d.kind === "purchase") add(bought, d.item, d.units, d.units * d.unit_price + d.fees);
  }
  const oldest = st.transactions.entries.at(-1);
  const partial = st.transactions.nextBefore !== null && !!oldest && oldest.reported_at > since;

  const happened = [tallyLine("Sold", sold), tallyLine("Bought", bought)].filter((line): line is string => !!line);
  const arrived = st.timeline.filter((entry) => entry.at_time > since);
  for (const entry of arrived) {
    const reward = /^Charter milestone · (.+) · the Authority paid ([\d,.]+) Cr\.$/.exec(entry.text);
    if (reward) happened.push(`${reward[1]} · +${Number(reward[2].replace(/,/g, "")).toLocaleString()} Cr`);
  }
  const bad = arrived.filter((entry) => entry.severity === "bad").length;
  if (bad) happened.push(`${bad} report${bad === 1 ? "" : "s"} need${bad === 1 ? "s" : ""} attention in the Log`);

  const plan = foundingPlan(st, home);
  const report = { away, happened, underway: plan?.progress ?? [], next: plan?.primary ?? null, partial };
  return report.happened.length || report.underway.length || report.next ? report : null;
}

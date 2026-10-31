// §contract-board (Oct 19 2026, Peter): the day's Authority contracts, after
// Travian's daily quests without the trap. Earned rewards are paid as reports
// arrive; untaken offers lapse at the day's end; an accepted one keeps a day.
import type { OperationView } from "../../protocol";
import type { ViewState } from "../../state";
import { fmt } from "./format";
import { fmtDays } from "./medals";

export interface BoardSummary {
  day: number;
  done: number;
  needed: number;
  /** §standing-ladder (R3): the bonus's standing and dossier fraction. */
  bonusStanding: number;
  bonusDossier: number;
  bonusPaid: boolean;
  /// Until the next day's contracts.
  nextIn: number;
  /// Today's contracts, open ones first.
  contracts: OperationView[];
}

const ORDER: Record<string, number> = { active: 0, offered: 1, completed: 2 };

/** The board, or null before the charter is fulfilled. */
export function boardSummary(st: ViewState, now = st.simTime): BoardSummary | null {
  const b = st.board;
  if (!b) return null;
  const contracts = st.operations.filter((o) => o.board_day === b.day)
    .sort((x, y) => (ORDER[x.state] ?? 3) - (ORDER[y.state] ?? 3) || String(x.id).localeCompare(String(y.id), undefined, { numeric: true }));
  return { day: b.day, done: b.done, needed: b.needed, bonusStanding: b.bonus_standing, bonusDossier: b.bonus_dossier, bonusPaid: b.bonus_paid,
    nextIn: Math.max(0, b.next_at - now), contracts };
}

/** "2/3 for +3 standing · 5% research · new in 6h 20m". */
export function boardLine(s: BoardSummary): string {
  const bonus = s.bonusPaid ? "Bonus paid" : `${Math.min(s.done, s.needed)}/${s.needed} for ${serviceReward(s.bonusStanding, s.bonusDossier)}`;
  return `${bonus} · new in ${fmtDays(s.nextIn)}`;
}

/** §standing-ladder (R3): "+3 standing · 5% research". */
export function serviceReward(standing: number, research: number): string {
  const parts = [];
  if (standing > 0) parts.push(`+${fmt(standing)} standing`);
  if (research > 0) parts.push(`${Math.round(research * 100)}% research`);
  return parts.join(" · ");
}

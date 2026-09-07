import type { BattleRecordView, BattleReportView, CaptureReportView, EntityId } from "./protocol";
import type { ViewState } from "./state";

type MarkState = Pick<ViewState, "galaxy" | "playerId" | "battleRecords" | "battleReports" | "captureReports" | "battleViewed" | "battleDismissed">;
type MarkStorage = Pick<Storage, "getItem" | "setItem">;

export const battleRecordMarkKey = (id: EntityId): string => `battle:${id}`;

export function reportMarkKey(report: BattleReportView | CaptureReportView): string {
  if ("captor" in report) return `capture:${report.id}`;
  return report.battle_id ? battleRecordMarkKey(report.battle_id) : `report:${report.id}`;
}

// Reports have their own counter; only the explicit engagement id may join
// them to replays. Missing legacy ids fail visibly, never hide another fight
// or open its replay because it happened at the same coordinates/time.
export function recordForBattleReport(report: BattleReportView, records: BattleRecordView[]): BattleRecordView | undefined {
  return report.battle_id ? records.find((rec) => rec.id === report.battle_id) : undefined;
}

export function reportForBattleRecord(record: BattleRecordView, reports: BattleReportView[]): BattleReportView | undefined {
  return reports.find((report) => report.battle_id === record.id);
}

function storageKey(st: MarkState): string | null {
  const instance = st.galaxy?.instance_id;
  // A seed, position or report number can repeat in a fresh galaxy. An older
  // server without an instance id gets session-only marks, not a guessed scope.
  return instance && st.playerId !== null
    ? `ss_battle_marks_v2:${JSON.stringify([instance, st.playerId])}` : null;
}

export function loadBattleMarks(st: MarkState, storage?: MarkStorage): void {
  // Called on EVERY Welcome, including reconnects and corporation switches.
  st.battleViewed = new Set();
  st.battleDismissed = new Set();
  try {
    const key = storageKey(st);
    if (!key) return;
    const raw = (storage ?? localStorage).getItem(key);
    if (!raw) return;
    const saved = JSON.parse(raw) as { viewed?: unknown; dismissed?: unknown };
    const marks = (value: unknown): Set<string> => new Set(Array.isArray(value)
      ? value.filter((entry): entry is string => typeof entry === "string" && /^(battle|capture|report):[0-9]+$/.test(entry)) : []);
    st.battleViewed = marks(saved.viewed);
    st.battleDismissed = marks(saved.dismissed);
  } catch { /* unavailable/corrupt storage leaves markers visible */ }
  // Deliberately do not migrate ss_battle_marks: its unscoped numeric ids
  // cannot be attributed to a player, galaxy or engagement safely.
}

export function saveBattleMarks(st: MarkState, storage?: MarkStorage): void {
  try {
    const key = storageKey(st);
    if (!key) return;
    const retained = new Set([
      ...st.battleRecords.map((rec) => battleRecordMarkKey(rec.id)),
      ...st.battleReports.map(reportMarkKey), ...st.captureReports.map(reportMarkKey),
    ]);
    const keep = (marks: Set<string>) => [...marks].filter((mark) => retained.has(mark));
    (storage ?? localStorage).setItem(key, JSON.stringify({ viewed: keep(st.battleViewed), dismissed: keep(st.battleDismissed) }));
  } catch { /* storage failure must not break the current dismissal or route */ }
}

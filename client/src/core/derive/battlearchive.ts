// §battle-archive: the Log's Battles list (both shells). A concluded battle
// leaves the galaxy map after a while (render.ts BATTLE_HISTORY_TTL_S); every
// record the client still holds is listed here, newest first, worded from the
// viewer's side.
import type { BattleRecordView, PlayerId } from "../../protocol";
import type { ViewState } from "../../state";
import { fmtBuildDur } from "./format";
import { locName, systemName } from "./geo";

export type ArchiveTone = "good" | "bad" | "quiet" | "live";

export interface ArchiveRow {
  id: string;
  ago: string;
  where: string;
  /** "You vs Pirates": the viewer's side first when it fought. */
  sides: string;
  outcome: string;
  tone: ArchiveTone;
  running: boolean;
}

type ArchiveState = Pick<ViewState, "playerId" | "galaxy" | "rankings" | "diplomacy" | "syndicate" | "battleRecords" | "battles">;

/** Notes that mean a side began leaving the fight. */
const LEAVING = new Set(["retreat_tripped", "withdraw_ordered", "disengage_exposure", "mutual_disengage"]);

/** A side's corporation, as the viewer knows it. */
export function corpLabel(st: ArchiveState, id: PlayerId): string {
  if (id === st.playerId) return "You";
  if (st.galaxy?.pirate_id !== undefined && id === st.galaxy.pirate_id) return "Pirates";
  return st.rankings.find((row) => row.player_id === id)?.name
    ?? st.diplomacy?.relations.find((relation) => relation.other === id)?.name
    ?? st.syndicate?.members.find((member) => member.id === id)?.name
    ?? "Rival";
}

/** The outcome from the viewer's side: Won, Lost, Withdrew… or a neutral
 * account when it only watched. */
export function battleVerdict(record: BattleRecordView): { label: string; tone: ArchiveTone } {
  const { outcome, own_side: own } = record;
  if (outcome === null) return { label: "Ongoing", tone: "live" };
  if (own === null) {
    const label = outcome === "attacker_destroyed" ? "Defender won"
      : outcome === "target_destroyed" ? "Attacker won"
        : outcome === "both_destroyed" ? "Both destroyed"
          : outcome === "escaped" ? "Escaped" : "Both survived";
    return { label, tone: "quiet" };
  }
  if ((own === 0 && outcome === "target_destroyed") || (own === 1 && outcome === "attacker_destroyed")) return { label: "Won", tone: "good" };
  if ((own === 0 && outcome === "attacker_destroyed") || (own === 1 && outcome === "target_destroyed")) return { label: "Lost", tone: "bad" };
  if (outcome === "both_destroyed") return { label: "Both destroyed", tone: "bad" };
  if (outcome === "escaped") return own === 1 ? { label: "Escaped", tone: "good" } : { label: "Target escaped", tone: "quiet" };
  const withdrew = record.rounds.some((round) => round.notes.some((note) => LEAVING.has(note.kind)
    && (note.side === own || note.kind === "mutual_disengage")));
  return { label: withdrew ? "Withdrew" : "Both survived", tone: "quiet" };
}

/** Every held record, newest first. */
export function archiveRows(st: ArchiveState, now: number): ArchiveRow[] {
  return [...st.battleRecords]
    .sort((a, b) => b.started_at - a.started_at || (a.id < b.id ? 1 : a.id > b.id ? -1 : 0))
    .map((record) => {
      const verdict = battleVerdict(record);
      const names = record.sides.map((side) => corpLabel(st, side.corp));
      const ordered = record.own_side === 1 ? [names[1], names[0]] : names;
      return {
        id: record.id,
        ago: `${fmtBuildDur(Math.max(0, now - record.started_at))} ago`,
        where: record.system ? systemName(record.system) : `Near ${locName(record.pos)}`,
        sides: `${ordered[0]} vs ${ordered[1]}`,
        outcome: verdict.label,
        tone: verdict.tone,
        running: record.outcome === null || st.battles.some((battle) => battle.id === record.id),
      };
    });
}

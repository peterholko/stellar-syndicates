import type { BattleRecordView, EntityId, RaidOutcome, RaidReport } from "../../protocol";
import { shipKindLabel } from "./fleet";

/** A win, a loss, or news that is neither. Both shells map it to their tone. */
export type BattleNoticeTone = "good" | "bad" | "quiet";

export interface BattleNotice {
  /** Short verdict for a notice title: "Victory", "Loss", or "Battle ended". */
  title: string;
  text: string;
  tone: BattleNoticeTone;
}

const verdict = (tone: BattleNoticeTone): string => tone === "good" ? "Victory" : tone === "bad" ? "Loss" : "Battle ended";

/** §founding R7: a delayed combat report worded from the reader's side. The
 * scripted founding Privateer is named, so its defeat reads as the lesson it is. */
export function reportNotice(report: RaidReport, pirateId?: string, privateer?: EntityId | null): BattleNotice {
  const mine = report.you === "attacker" ? report.attacker_kind : report.target_kind;
  const theirs = report.you === "attacker" ? report.target_kind : report.attacker_kind;
  const theirShip = report.you === "attacker" ? report.target_ship : report.attacker_ship;
  const yourShipDied = report.outcome === "both_destroyed"
    || (report.you === "attacker" && report.outcome === "attacker_destroyed")
    || (report.you === "defender" && report.outcome === "target_destroyed");
  let text: string;
  let tone: BattleNoticeTone = "good";
  switch (report.outcome) {
    case "both_destroyed":
      text = `Your ${shipKindLabel(mine)} and a rival ${shipKindLabel(theirs)} destroyed each other.`;
      tone = "bad";
      break;
    case "both_survive":
      text = report.you === "attacker"
        ? `Your raid on a rival ${shipKindLabel(theirs)} was driven off — both survived.`
        : `A raider attacked your ${shipKindLabel(mine)} but was driven off.`;
      tone = report.you === "attacker" ? "quiet" : "good";
      break;
    case "escaped":
      text = report.you === "attacker"
        ? `Your target ${shipKindLabel(theirs)} reached the hub — raid failed.`
        : `Your ${shipKindLabel(mine)} reached the hub safely.`;
      tone = report.you === "attacker" ? "quiet" : "good";
      break;
    default:
      if (yourShipDied) {
        text = `Your ${shipKindLabel(mine)} was destroyed by a rival ${shipKindLabel(theirs)}.`;
        tone = "bad";
      } else {
        text = `Your ${shipKindLabel(mine)} destroyed a rival ${shipKindLabel(theirs)}.`;
      }
  }
  if (privateer && theirShip === privateer) text = text.replace(`a rival ${shipKindLabel(theirs)}`, "the Rogue Privateer");
  else if (pirateId && (report.attacker === pirateId || report.defender === pirateId)) text = text.replaceAll("rival", "pirate");
  return { title: verdict(tone), text, tone };
}

/** A concluded battle record, worded from the viewer's side when it fought. */
export function battleNotice(outcome: RaidOutcome, record?: BattleRecordView): BattleNotice {
  const ownSide = record?.own_side;
  const notice = (text: string, tone: BattleNoticeTone): BattleNotice => ({ title: verdict(tone), text, tone });
  switch (outcome) {
    case "both_destroyed": return notice("Both fleets destroyed each other.", "bad");
    case "both_survive": return notice("Both fleets survived.", "quiet");
    case "escaped": return notice("The target escaped.", ownSide === 1 ? "good" : "quiet");
    case "attacker_destroyed":
      return ownSide === 0
        ? notice("Your attacking fleet was destroyed.", "bad")
        : ownSide === 1
          ? notice("Your fleet destroyed the attacker.", "good")
          : notice("The attacking fleet was destroyed.", "quiet");
    case "target_destroyed":
      return ownSide === 1
        ? notice("Your defending fleet was destroyed.", "bad")
        : ownSide === 0
          ? notice("Your fleet destroyed the defender.", "good")
          : notice("The defending fleet was destroyed.", "quiet");
  }
}

import type {
  Commodity,
  EngagementEstimate,
  EntityId,
  OrderKind,
  PlayerId,
  RaidOutcome,
  RaidReport,
  TimelineEntry,
  TradeEvent,
} from "../protocol";
import type { LinkStatus, PendingIntent } from "../state";
import type { VendettaBeat } from "./derive/vendetta";

export type CoreEvent =
  | { kind: "LinkChanged"; status: LinkStatus }
  | { kind: "Welcomed"; playerId: PlayerId; name: string }
  | { kind: "SessionReplaced" }
  | { kind: "ProtocolMismatch"; server: number; client: number }
  | { kind: "GalaxyUpdated" }
  | { kind: "ViewApplied" }
  | { kind: "BattleRecordsApplied" }
  | { kind: "GroundRecordsApplied" }
  | { kind: "SectionsApplied" }
  | { kind: "CommandSignal"; orderId: number; shipId: EntityId }
  | { kind: "CommandChevron"; fleetId?: EntityId }
  | { kind: "OrderConfirmed"; orderId: number; shipId: EntityId; orderKind: OrderKind }
  | { kind: "FleetDocked"; fleetId: EntityId; berth: EntityId }
  | { kind: "FleetArrived"; fleetId: EntityId }
  | { kind: "BuildCompleted"; systemId: EntityId; buildKey: string; bodyId?: number }
  | { kind: "StructureStaffed"; systemId: EntityId; title: string }
  | { kind: "ResearchCompleted"; programmeId: string; programmeName: string }
  | { kind: "CommandRejected"; message: string }
  /** §log: journal news that toasts from its kind (a home raid's warning,
   * start and end); the shell words it with `logToast`. */
  | { kind: "LogToast"; entry: TimelineEntry }
  /** §signals: a signal detected near home, or a sweep's report in. */
  | { kind: "SignalNews"; title: string; message: string; tone: "good" | "quiet" }
  /** §founding R6: an Authority stipend or bounty arrived. */
  | { kind: "CharterMilestone"; title: string; message: string; tone?: "good" | "bad" }
  /** §charter-v5: a story turn shown as an illustrated pop-up (boot.ts):
   * the raid's report, with what it took. */
  | { kind: "StoryEvent"; story: "pirates"; goods: Partial<Record<Commodity, number>> }
  /** §charter-v4: the free trader docked at home (the server's arrival line). */
  | { kind: "StoryEvent"; story: "free_trader" }
  /** §den-loot: the den's fall, its loot waiting for a freighter. */
  | { kind: "StoryEvent"; story: "den_loot" }
  /** §den-contract: the Authority's contract on the privateer's den, with
   * the kill's report ("Authority contract · {title} · {reward}."). */
  | { kind: "StoryEvent"; story: "den_contract"; title: string; reward: string }
  /** §privateer-contract: the Authority's contract on the privateer, offered
   * as the hunt begins ("Authority contract offered · {title} · {reward}."). */
  | { kind: "StoryEvent"; story: "privateer_contract"; title: string; reward: string }
  /** §vendetta: a turn of the pirate family's story (a "Vendetta · …" line,
   * `text` without its prefix). */
  | { kind: "StoryEvent"; story: "vendetta"; beat: Exclude<VendettaBeat, "raid">; text: string }
  /** §season: an Authority decree. */
  | { kind: "StoryEvent"; story: "decree"; grip: import("../protocol").Grip }
  /** §season: a crackdown that strikes you or everyone, or a false flag exposed. */
  | { kind: "StoryEvent"; story: "crackdown"; crackdown: import("../protocol").CrackdownKind; at: number }
  | { kind: "StoryEvent"; story: "exposed"; text: string; at: number }
  /** §season phase 4: a sealed world found, the spy's leak, war with the Authority. */
  | { kind: "StoryEvent"; story: "journey"; beat: "found" | "spy" | "war"; text: string; at: number }
  /** §season phase 5: the season's end. */
  | { kind: "StoryEvent"; story: "ending"; at: number }
  /** §season phase 4: the GNN's "The relics stir", first heard this session. */
  | { kind: "StoryEvent"; story: "relics" }
  /** §signal-report: a signal's report reached command. */
  | { kind: "StoryEvent"; story: "signal"; signalId: EntityId }
  /** §chapters: a painted chapter of the charter opens. */
  | { kind: "StoryEvent"; story: "chapter"; chapter: ChapterOpening }
  | { kind: "ReportArrived"; report: RaidReport }
  | { kind: "BattleConcluded"; recordId: EntityId; outcome: RaidOutcome }
  | { kind: "EstimateReady"; estimate: EngagementEstimate }
  | { kind: "TimelineApplied" }
  | { kind: "TradeSettled"; trade: TradeEvent }
  | { kind: "TransactionsApplied" }
  | {
      kind: "IntentChanged";
      intent: PendingIntent | null;
      jumpAiming: string | null;
      guardAiming: string | null;
      readout?: string;
      renderIntentBar?: boolean;
      refreshShip?: boolean;
    }
  | { kind: "TransportError"; url: string }
  | { kind: "JoinRejected"; message: string }
  | { kind: "ServerError"; message: string };

/** §chapters (Oct 29 2026): the charter's chapters that open with a painting
 * of their own (Landfall has the prologue, the Raid and the Vendetta theirs). */
export type ChapterOpening = "pirates" | "expansion" | "reach";

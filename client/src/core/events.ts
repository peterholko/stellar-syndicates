import type {
  EngagementEstimate,
  EntityId,
  OrderKind,
  PlayerId,
  RaidOutcome,
  RaidReport,
  TradeEvent,
} from "../protocol";
import type { LinkStatus, PendingIntent } from "../state";

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
  | { kind: "PirateRaidWarning"; message: string }
  /** §signals: a signal detected near home, or a sweep's report in. */
  | { kind: "SignalNews"; title: string; message: string; tone: "good" | "quiet" }
  /** §founding R6: an Authority stipend or bounty arrived. */
  | { kind: "CharterMilestone"; title: string; message: string; tone?: "good" | "bad" }
  /** §charter-v5: a story turn shown as an illustrated pop-up (boot.ts):
   * the raid's report, with what it took. */
  | { kind: "StoryEvent"; story: "pirates"; semiconductors: number; armaments: number }
  /** §charter-v4: the free trader docked at home (the server's arrival line). */
  | { kind: "StoryEvent"; story: "free_trader" }
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

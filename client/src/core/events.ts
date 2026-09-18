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
  | { kind: "BuildCompleted"; systemId: EntityId; buildKey: string }
  | { kind: "StructureStaffed"; systemId: EntityId; title: string }
  | { kind: "ResearchCompleted"; programmeId: string; programmeName: string }
  | { kind: "CommandRejected"; message: string }
  | { kind: "PirateRaidWarning"; message: string }
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

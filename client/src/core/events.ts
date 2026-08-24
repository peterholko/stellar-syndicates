import type {
  EngagementEstimate,
  EntityId,
  OrderKind,
  PlayerId,
  RaidOutcome,
  RaidReport,
  TradeEvent,
} from "../protocol";
import type { LinkStatus } from "../state";

export type CoreEvent =
  | { kind: "LinkChanged"; status: LinkStatus }
  | { kind: "Welcomed"; playerId: PlayerId; name: string }
  | { kind: "ProtocolMismatch"; server: number; client: number }
  | { kind: "GalaxyUpdated" }
  | { kind: "ViewApplied" }
  | { kind: "BattleRecordsApplied" }
  | { kind: "GroundRecordsApplied" }
  | { kind: "SectionsApplied" }
  | { kind: "CommandSignal"; orderId: number; shipId: EntityId }
  | { kind: "CommandChevron"; fleetId?: EntityId }
  | { kind: "OrderConfirmed"; orderId: number; shipId: EntityId; orderKind: OrderKind }
  | { kind: "ReportArrived"; report: RaidReport }
  | { kind: "BattleConcluded"; recordId: EntityId; outcome: RaidOutcome }
  | { kind: "EstimateReady"; estimate: EngagementEstimate }
  | { kind: "TimelineApplied" }
  | { kind: "TradeSettled"; trade: TradeEvent }
  | { kind: "JoinRejected"; message: string }
  | { kind: "ServerError"; message: string };

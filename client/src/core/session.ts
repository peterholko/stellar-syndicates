import type {
  BattleRecordView,
  EngagementEstimate,
  GroundRecordView,
  JumpDepartureView,
  ServerMsg,
} from "../protocol";
import {
  JUMP_DEPARTURE_TTL_S,
  syncRenderClock,
  type LinkStatus,
  type ViewState,
} from "../state";
import type { CoreEvent } from "./events";
import { noteSurveyReports } from "./derive/geo";
import {
  marketReservations,
  recentMarketOrders,
  recordPriceHistory,
} from "./derive/market";
import { syncOrderLifecycles } from "./derive/orders";
import { mergeResearch } from "./derive/research";

export const EXPECTED_PROTOCOL_VERSION = 29;

export const jumpDepartureKey = (
  departure: Pick<JumpDepartureView, "fleet" | "departed_at">,
): string => `${departure.fleet}:${departure.departed_at.toFixed(6)}`;

export function applyLinkStatus(status: LinkStatus, st: ViewState): CoreEvent[] {
  st.link = status;
  return [{ kind: "LinkChanged", status }];
}

export function applySessionReplaced(st: ViewState): CoreEvent[] {
  st.playerId = null;
  st.link = "offline";
  return [{ kind: "SessionReplaced" }];
}

// The server stream is reduced into the one shared, served ViewState here.
// This layer deliberately has no DOM: shells receive typed CoreEvents for every
// presentational consequence and may render the same state in different forms.
export function applyServerMessage(msg: ServerMsg, st: ViewState): CoreEvent[] {
  switch (msg.type) {
    case "Welcome": {
      const events: CoreEvent[] = [];
      marketReservations.length = 0;
      if (st.playerId !== null && st.playerId !== msg.player_id) recentMarketOrders.length = 0;
      if (typeof msg.protocol_version === "number" && msg.protocol_version !== EXPECTED_PROTOCOL_VERSION) {
        events.push({
          kind: "ProtocolMismatch",
          server: msg.protocol_version,
          client: EXPECTED_PROTOCOL_VERSION,
        });
      }
      st.playerId = msg.player_id;
      st.name = msg.name;
      st.tickHz = msg.tick_hz;
      st.pacingScale = msg.pacing_scale;
      st.tick = msg.tick;
      syncRenderClock(msg.sim_time, st.pacingScale);
      st.simTime = msg.sim_time;
      st.galaxy = msg.galaxy;
      st.charterLadder = msg.charter_ladder;
      st.researchCatalog = msg.research_catalog;
      st.battleRecords = st.battleRecords.filter((record) => record.id === "demo-battle");
      st.standingOrders = [];
      st.battleReports = [];
      st.captureReports = [];
      st.rankings = [];
      st.link = "online";
      st.awaySet = false;
      events.push({ kind: "Welcomed", playerId: msg.player_id, name: msg.name });
      return events;
    }

    case "GalaxyUpdate":
      if (st.galaxy) st.galaxy = { ...st.galaxy, systems: msg.systems };
      return [{ kind: "GalaxyUpdated" }];

    case "View": {
      st.tick = msg.tick;
      syncRenderClock(msg.sim_time, st.pacingScale);
      st.simTime = msg.sim_time;
      st.commandCenter = msg.command_center;
      st.anchors = msg.anchors;
      st.systems = msg.systems;
      st.ghosts = msg.ghosts;
      const ownFleetIds = new Set(msg.ghosts.filter((ghost) => ghost.own).map((ghost) => ghost.id));
      for (const id of st.selectedShipIds) if (!ownFleetIds.has(id)) st.selectedShipIds.delete(id);
      if (st.selectedShipId && !msg.ghosts.some((ghost) => ghost.id === st.selectedShipId)) {
        st.selectedShipId = st.selectedShipIds.values().next().value ?? null;
        st.selectedOrderId = null;
      }
      st.captains = msg.captains ?? [];
      st.captainCapacity = msg.captain_capacity ?? 1;
      const departures = new Map(
        st.jumpDepartures.map((departure) => [jumpDepartureKey(departure), departure]),
      );
      for (const departure of msg.jump_departures ?? []) {
        departures.set(jumpDepartureKey(departure), departure);
      }
      st.jumpDepartures = [...departures.values()].filter(
        (departure) => msg.sim_time - departure.learned_at < JUMP_DEPARTURE_TTL_S,
      );
      st.emplacements = msg.emplacements ?? [];
      st.market = msg.market;
      st.wallet = msg.wallet;
      st.freight = msg.freight;
      st.charter = msg.charter;
      st.founding = msg.founding;
      st.doctrine = msg.doctrine;
      st.battles = msg.battles;
      st.syndicate = msg.syndicate ?? null;
      st.syndicateInvites = msg.syndicate_invites ?? [];
      st.operations = msg.operations ?? [];
      st.midgameStage = msg.midgame_stage;
      st.diplomacy = msg.diplomacy ?? null;
      st.research = msg.research ? mergeResearch(msg.research, st) : null;
      noteSurveyReports(msg.sim_time, st);
      syncOrderLifecycles(msg.pending_orders, msg.sim_time, st);

      for (const [id, dest] of Object.entries(st.orders)) {
        const ghost = st.ghosts.find((candidate) => candidate.id === id && candidate.own);
        if (!ghost) continue;
        const parked = Math.hypot(ghost.vel.x, ghost.vel.y) < 0.5;
        if (parked && Math.hypot(ghost.pos.x - dest.x, ghost.pos.y - dest.y) < 500) {
          delete st.orders[id];
        }
      }
      recordPriceHistory(st);
      st.corpsInView = new Set(msg.ghosts.map((ghost) => ghost.owner)).size;
      st.link = "online";
      return [{ kind: "ViewApplied" }];
    }

    case "BattleRecords": {
      let records = st.battleRecords;
      const concluded: CoreEvent[] = [];
      for (const id of msg.removed ?? []) records = records.filter((record) => record.id !== id);
      for (const update of msg.updates ?? []) {
        const previous = records.find((record) => record.id === update.id);
        const base: BattleRecordView | undefined = update.header
          ? {
              id: update.id,
              ...update.header,
              rounds: previous?.rounds ?? [],
              light_frontier_tick: update.light_frontier_tick,
              outcome: previous?.outcome ?? null,
            }
          : previous;
        if (!base) continue;
        const next: BattleRecordView = {
          ...base,
          rounds: update.new_rounds?.length
            ? [...base.rounds, ...update.new_rounds]
            : base.rounds,
          light_frontier_tick: update.light_frontier_tick,
          outcome: update.outcome ?? base.outcome ?? null,
        };
        records = previous
          ? records.map((record) => (record.id === update.id ? next : record))
          : records.concat([next]);
        if (previous?.outcome == null && next.outcome !== null) {
          concluded.push({ kind: "BattleConcluded", recordId: next.id, outcome: next.outcome });
        }
      }
      st.battleRecords = records;
      return [{ kind: "BattleRecordsApplied" }, ...concluded];
    }

    case "GroundRecords": {
      let records = st.groundRecords;
      for (const id of msg.removed ?? []) records = records.filter((record) => record.id !== id);
      for (const update of msg.updates ?? []) {
        const previous = records.find((record) => record.id === update.id);
        const base: GroundRecordView | undefined = update.header
          ? {
              id: update.id,
              ...update.header,
              rounds: previous?.rounds ?? [],
              light_frontier_tick: update.light_frontier_tick,
              outcome: previous?.outcome ?? null,
            }
          : previous;
        if (!base) continue;
        const next: GroundRecordView = {
          ...base,
          rounds: update.new_rounds?.length
            ? [...base.rounds, ...update.new_rounds]
            : base.rounds,
          light_frontier_tick: update.light_frontier_tick,
          outcome: update.outcome ?? base.outcome ?? null,
        };
        records = previous
          ? records.map((record) => (record.id === update.id ? next : record))
          : records.concat([next]);
      }
      st.groundRecords = records;
      return [{ kind: "GroundRecordsApplied" }];
    }

    case "Sections":
      if (msg.standing_orders) st.standingOrders = msg.standing_orders;
      if (msg.battle_reports) st.battleReports = msg.battle_reports;
      if (msg.capture_reports) st.captureReports = msg.capture_reports;
      if (msg.rankings) st.rankings = msg.rankings;
      return [{ kind: "SectionsApplied" }];

    case "CommandSignal":
      st.commandSignals = st.commandSignals.filter((signal) => signal.orderId !== msg.order_id);
      st.commandSignals.push({
        orderId: msg.order_id,
        shipId: msg.ship_id,
        targetPos: undefined,
        depart: msg.depart_time,
        arrive: msg.arrive_time,
        pOut: 0,
        hops: msg.hops ?? [],
      });
      return [{ kind: "CommandSignal", orderId: msg.order_id, shipId: msg.ship_id }];

    case "CommandChevron":
      st.commandSignals.push({
        orderId: 0,
        shipId: msg.fleet_id ?? "",
        targetPos: msg.target_pos,
        depart: msg.depart_time,
        arrive: msg.arrive_time,
        pOut: 0,
        hops: [],
      });
      return [{ kind: "CommandChevron", fleetId: msg.fleet_id }];

    case "OrderConfirmed":
      st.commandSignals = st.commandSignals.filter((signal) => signal.orderId !== msg.order_id);
      return [{
        kind: "OrderConfirmed",
        orderId: msg.order_id,
        shipId: msg.ship_id,
        orderKind: msg.kind,
      }];

    case "Report":
      delete st.raids[msg.report.attacker_ship];
      return [{ kind: "ReportArrived", report: msg.report }];

    case "EngagementEstimate": {
      const estimate: EngagementEstimate = msg;
      return [{ kind: "EstimateReady", estimate }];
    }

    case "Timeline":
      st.timeline = msg.entries;
      if (!st.awaySet) {
        st.awaySince = msg.away_since;
        st.awaySet = true;
      }
      return [{ kind: "TimelineApplied" }];

    case "Trade":
      return [{ kind: "TradeSettled", trade: msg.trade }];

    case "Error":
      return st.playerId === null
        ? [{ kind: "JoinRejected", message: msg.message }]
        : [{ kind: "ServerError", message: msg.message }];
  }
}

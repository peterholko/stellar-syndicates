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
import type { ChapterOpening, CoreEvent } from "./events";
import { loadBattleMarks } from "../battlehistory";
import { chartStars } from "../stars";
import { crackdownOf, decreeOf } from "./derive/season";
import { vendettaBeatOf } from "./derive/vendetta";
import { noteSurveyReports } from "./derive/geo";
import {
  marketReservations,
  recentMarketOrders,
  recordPriceHistory,
} from "./derive/market";
import { syncOrderLifecycles } from "./derive/orders";
import { mergeResearch } from "./derive/research";
import { noteStockSamples } from "./derive/stores";
import { applyTransactions, emptyTransactions } from "./derive/transactions";

import { PROTOCOL_VERSION } from "../wire.mjs";

export const EXPECTED_PROTOCOL_VERSION = PROTOCOL_VERSION;

export const jumpDepartureKey = (
  departure: Pick<JumpDepartureView, "fleet" | "departed_at">,
): string => `${departure.fleet}:${departure.departed_at.toFixed(6)}`;

/** §chapters: the milestone lines that open the charter's painted chapters
 * (`founding::charter_stipend`'s titles). */
const CHAPTER_OPENINGS: Record<string, ChapterOpening> = {
  "Shipyard staffed": "pirates", "Privateer den eliminated": "expansion", "Nearby system charted": "reach",
};

export function applyLinkStatus(status: LinkStatus, st: ViewState): CoreEvent[] {
  st.link = status;
  if (status !== "online") st.transactions.loading = false;
  return [{ kind: "LinkChanged", status }];
}

export function applySessionReplaced(st: ViewState): CoreEvent[] {
  st.playerId = null;
  st.transactions = emptyTransactions();
  st.link = "offline";
  return [{ kind: "SessionReplaced" }];
}

// The server stream is reduced into the one shared, served ViewState here.
// This layer deliberately has no DOM: shells receive typed CoreEvents for every
// presentational consequence and may render the same state in different forms.
export function applyServerMessage(msg: ServerMsg, st: ViewState): CoreEvent[] {
  switch (msg.type) {
    case "FoundingOffer": return []; // the shared boot-level introduction owns presentation
    case "InviteStatus": return []; // §settle-near-friends: the landing dialog shows it
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
      st.transactions = emptyTransactions();
      st.name = msg.name;
      st.tickHz = msg.tick_hz;
      st.pacingScale = msg.pacing_scale;
      st.tick = msg.tick;
      syncRenderClock(msg.sim_time, st.pacingScale);
      st.simTime = msg.sim_time;
      st.galaxy = msg.galaxy;
      chartStars(msg.galaxy.systems);
      st.charterLadder = msg.charter_ladder;
      st.gradeLadder = msg.grade_ladder ?? [];
      st.lastFoundingPrivateer = null;
      st.researchCatalog = msg.research_catalog;
      st.battleRecords = st.battleRecords.filter((record) => record.id === "demo-battle");
      st.standingOrders = [];
      st.explorationSites = [];
      st.selectedExplorationSiteId = null;
      st.explorationJournal = [];
      st.battleReports = [];
      st.captureReports = [];
      loadBattleMarks(st);
      st.rankings = [];
      st.syndicateRankings = [];
      st.link = "online";
      st.awaySet = false;
      st.storyTold = new Set();
      st.season = null;
      st.homeOpened = false;
      st.homeWorldOpened = false;
      events.push({ kind: "Welcomed", playerId: msg.player_id, name: msg.name });
      return events;
    }

    case "GalaxyUpdate":
      if (st.galaxy) st.galaxy = { ...st.galaxy, systems: msg.systems };
      chartStars(msg.systems);
      return [{ kind: "GalaxyUpdated" }];

    case "View": {
      const hadServedView = st.systems.length > 0 || st.ghosts.length > 0;
      const previousGhosts = new Map(st.ghosts.map((ghost) => [ghost.id, ghost]));
      const previousSystems = new Map(st.systems.map((system) => [system.id, system]));
      const previousOrders = { ...st.orders };
      const previousCompletedResearch = new Set(
        st.research?.programmes.filter((programme) => programme.state === "completed").map((programme) => programme.id) ?? [],
      );
      st.tick = msg.tick;
      syncRenderClock(msg.sim_time, st.pacingScale);
      st.simTime = msg.sim_time;
      st.commandCenter = msg.command_center;
      st.anchors = msg.anchors;
      st.systems = msg.systems;
      // §stores: rates are measured from successive stockpiles.
      noteStockSamples(st, msg.sim_time);
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
      st.interdictionFields = msg.interdiction_fields ?? [];
      st.market = msg.market;
      // A legacy-save warm-up is unknown, not an empty bank account. Every
      // shell keeps its ordinary unavailable-report treatment until light arrives.
      st.wallet = msg.wallet.report_pending ? null : msg.wallet;
      st.credits = msg.wallet.credits;
      st.freight = msg.freight;
      st.charter = msg.charter;
      st.founding = msg.founding;
      st.charterLedger = msg.charter_ledger ?? null;
      st.leave = msg.leave ?? null;
      st.raidLosses = msg.raid_losses ?? null;
      st.merchant = msg.merchant ?? null;
      if (msg.founding?.privateer) st.lastFoundingPrivateer = msg.founding.privateer;
      st.doctrine = msg.doctrine;
      st.battles = msg.battles;
      st.syndicate = msg.syndicate ?? null;
      st.syndicateInvites = msg.syndicate_invites ?? [];
      st.operations = msg.operations ?? [];
      st.board = msg.board ?? null;
      st.signals = msg.signals ?? null;
      st.explorationSites = msg.exploration_sites ?? [];
      st.midgameStage = msg.midgame_stage;
      st.diplomacy = msg.diplomacy ?? null;
      st.research = msg.research ? mergeResearch(msg.research, st) : null;
      noteSurveyReports(msg.sim_time, st);
      syncOrderLifecycles(msg.pending_orders, msg.sim_time, st);

      const derived: CoreEvent[] = [];
      for (const ghost of st.ghosts) {
        if (!hadServedView || !ghost.own) continue;
        const previous = previousGhosts.get(ghost.id);
        if (ghost.docked && previous && previous.docked !== ghost.docked) {
          derived.push({ kind: "FleetDocked", fleetId: ghost.id, berth: ghost.docked });
          continue;
        }
        const dest = previousOrders[ghost.id];
        if (!dest || !previous) continue;
        const arrived = Math.hypot(ghost.pos.x - dest.x, ghost.pos.y - dest.y) < 500
          && Math.hypot(ghost.vel.x, ghost.vel.y) < 0.5;
        const wasArrived = Math.hypot(previous.pos.x - dest.x, previous.pos.y - dest.y) < 500
          && Math.hypot(previous.vel.x, previous.vel.y) < 0.5;
        if (arrived && !wasArrived) derived.push({ kind: "FleetArrived", fleetId: ghost.id });
      }
      if (hadServedView) {
        for (const system of st.systems) {
          const previous = previousSystems.get(system.id);
          if (!previous || system.owner !== st.playerId) continue;
          const remainingBuilds = new Map<string, number>();
          for (const build of system.builds) {
            const key = `${build.body_id}:${build.key}`;
            remainingBuilds.set(key, (remainingBuilds.get(key) ?? 0) + 1);
          }
          for (const build of previous.builds) {
            const key = `${build.body_id}:${build.key}`;
            const remaining = remainingBuilds.get(key) ?? 0;
            if (remaining > 0) remainingBuilds.set(key, remaining - 1);
            else if (build.complete_time != null && build.complete_time <= msg.sim_time + 1) {
              derived.push({ kind: "BuildCompleted", systemId: system.id, buildKey: build.key, bodyId: build.body_id });
            }
          }
          const previousAssignments = new Map(
            previous.assignments.map((assignment) => [`${assignment.body_id}:${assignment.structure}`, assignment]),
          );
          for (const assignment of system.assignments) {
            const old = previousAssignments.get(`${assignment.body_id}:${assignment.structure}`);
            if (assignment.workers > 0 && (old?.workers ?? 0) === 0) {
              derived.push({ kind: "StructureStaffed", systemId: system.id, title: assignment.title });
            }
          }
        }
        for (const programme of st.research?.programmes ?? []) {
          if (programme.state === "completed" && !previousCompletedResearch.has(programme.id)) {
            derived.push({ kind: "ResearchCompleted", programmeId: programme.id, programmeName: programme.name });
          }
        }
      }

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
      return [{ kind: "ViewApplied" }, ...derived];
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
      if (msg.exploration_journal) st.explorationJournal = msg.exploration_journal;
      if (msg.standing_orders) st.standingOrders = msg.standing_orders;
      if (msg.depot_rules) st.depotRules = msg.depot_rules;
      if (msg.battle_reports) st.battleReports = msg.battle_reports;
      if (msg.capture_reports) st.captureReports = msg.capture_reports;
      if (msg.rankings) st.rankings = msg.rankings;
      if (msg.syndicate_rankings) st.syndicateRankings = msg.syndicate_rankings;
      if (msg.social) st.social = msg.social;
      // §season: the season's end pops up when its news first arrives, and
      // so does the GNN's word that the relics wake (news heard before this
      // session was told then).
      const endedNow = !!msg.season?.ending && !!st.season && !st.season.ending;
      const heard = new Set(st.season?.news.map((item) => item.id));
      const stirred = !!st.season && !!msg.season?.news.some((item) => item.kind === "relic"
        && item.headline.startsWith("The relics stir") && !heard.has(item.id));
      if (msg.season) st.season = msg.season;
      const told: CoreEvent[] = [{ kind: "SectionsApplied" }];
      if (endedNow) told.push({ kind: "StoryEvent", story: "ending", at: msg.season!.ending!.at });
      if (stirred) told.push({ kind: "StoryEvent", story: "relics" });
      return told;

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

    case "Timeline": {
      const previous = new Set(st.timeline.map((entry) => `${entry.at_time}:${entry.severity}:${entry.text}`));
      const hadTimeline = st.awaySet;
      st.timeline = msg.entries;
      // §charter-v4: turns whose news predates this session were told then
      // (or met in the inbox while away): no pop-up, and nothing withheld.
      if (!hadTimeline && msg.entries.some((entry) => entry.text.startsWith("A free trader docked at "))) {
        st.storyTold.add("free_trader");
      }
      if (!hadTimeline && msg.entries.some((entry) => entry.text.startsWith("Pirates raided "))) {
        st.storyTold.add("pirates");
      }
      if (!hadTimeline) {
        for (const entry of msg.entries) {
          const beat = vendettaBeatOf(entry.text);
          if (beat) st.storyTold.add(`vendetta_${beat}`);
          const grip = decreeOf(entry.text);
          if (grip) st.storyTold.add(`decree_${grip}`);
        }
      }
      if (!st.awaySet) {
        st.awaySince = msg.away_since;
        st.awaySet = true;
        st.welcome = msg.welcome ?? null;
      }
      const rejected = hadTimeline
        ? msg.entries
            .filter((entry) => !previous.has(`${entry.at_time}:${entry.severity}:${entry.text}`))
            .filter((entry) => /^(order refused|can't build)/i.test(entry.text))
            .map((entry): CoreEvent => ({ kind: "CommandRejected", message: entry.text }))
        : [];
      // These notices are already propagation-gated by the server journal.
      // Never inspect hidden NPC orders or replay old warnings on reconnect.
      // §log (v71): a home raid's warning, start and end toast from the
      // entry's kind and phase, not its wording.
      const raids = hadTimeline ? msg.entries
        .filter((entry) => !previous.has(`${entry.at_time}:${entry.severity}:${entry.text}`)
          && entry.kind === "raid" && !!entry.group && !!entry.phase)
        .map((entry): CoreEvent => ({ kind: "LogToast", entry })) : [];
      // §founding R6: charter rewards are moments, never silent credits. The
      // server stamps them on the founder's own clock; only new lines toast.
      const milestones = hadTimeline ? msg.entries
        .filter((entry) => !previous.has(`${entry.at_time}:${entry.severity}:${entry.text}`))
        .map((entry): CoreEvent | null => {
          // §log: "Charter milestone · {title} · 500 Cr · +2 standing." (the
          // old "the Authority paid N Cr" never matched the server's line).
          const reward = /^Charter milestone · (.+?) · (.+)\.$/.exec(entry.text);
          if (reward) return { kind: "CharterMilestone", title: "Charter milestone", message: `${reward[1]} · ${reward[2]}` };
          // §charter-v5: the raid's report is a story pop-up, not a toast.
          const raid = /^Pirates raided .+ and took (.+)\.$/.exec(entry.text);
          if (raid) {
            const took = (good: string) => Number(new RegExp(`([\\d,]+) ${good}`).exec(raid[1])?.[1]?.replace(/,/g, "") ?? 0);
            return { kind: "StoryEvent", story: "pirates", semiconductors: took("semiconductors"), armaments: took("armaments") };
          }
          // The trader's docking is the first the founder hears of it.
          if (entry.text.startsWith("A free trader docked at ")) return { kind: "StoryEvent", story: "free_trader" };
          // §vendetta: each turn of the pirate family's story is a transmission.
          const beat = vendettaBeatOf(entry.text);
          if (beat) return { kind: "StoryEvent", story: "vendetta", beat, text: entry.text.slice("Vendetta · ".length) };
          // §season: each decree is the Authority's own transmission.
          const grip = decreeOf(entry.text);
          if (grip) return { kind: "StoryEvent", story: "decree", grip };
          // §season: a crackdown on you (or the Hub's sabotage), and a false flag exposed.
          const crackdown = crackdownOf(entry.text);
          if (crackdown && (crackdown === "sabotage" || entry.severity === "bad")) return { kind: "StoryEvent", story: "crackdown", crackdown, at: entry.at_time };
          if (entry.text.startsWith("False flag exposed · ")) return { kind: "StoryEvent", story: "exposed", text: entry.text, at: entry.at_time };
          // §season phase 4: the turns of the relic journey.
          if (entry.text.startsWith("Sealed world found · ")) return { kind: "StoryEvent", story: "journey", beat: "found", text: entry.text, at: entry.at_time };
          if (entry.text.startsWith("An Authority spy · ")) return { kind: "StoryEvent", story: "journey", beat: "spy", text: entry.text, at: entry.at_time };
          if (entry.text.startsWith("War with the Authority · ")) return { kind: "StoryEvent", story: "journey", beat: "war", text: entry.text, at: entry.at_time };
          if (entry.text.startsWith("The Seal fires · ") || entry.text.startsWith("The Charter holds · ")) return { kind: "StoryEvent", story: "ending", at: entry.at_time };
          // §signals: a new signal near home, and each report.
          const signal = /^New signal · (.+)\.$/.exec(entry.text);
          if (signal) return { kind: "SignalNews", title: "New signal", message: signal[1], tone: "quiet" };
          const find = /^(First find \d+\/\d+|Report in) · (.+)\.$/.exec(entry.text);
          if (find) return { kind: "SignalNews", title: find[1], message: find[2], tone: "good" };
          // §medals: rewards are moments too.
          const medal = /^Week (\d+) · (Gold|Silver|Bronze) medal · (.+)\.$/.exec(entry.text);
          if (medal) return { kind: "CharterMilestone", title: `${medal[2]} medal`, message: `Week ${medal[1]} · ${medal[3]}` };
          return null;
        })
        .filter((event): event is CoreEvent => event !== null) : [];
      // §chapters: three of the charter's chapters open with a painting, on
      // the milestone line that starts them (the toast still shows).
      const chapters = hadTimeline ? msg.entries
        .filter((entry) => !previous.has(`${entry.at_time}:${entry.severity}:${entry.text}`))
        .map((entry) => CHAPTER_OPENINGS[/^Charter milestone · (.+?) · /.exec(entry.text)?.[1] ?? ""])
        .filter((chapter): chapter is ChapterOpening => !!chapter)
        .map((chapter): CoreEvent => ({ kind: "StoryEvent", story: "chapter", chapter })) : [];
      return [{ kind: "TimelineApplied" }, ...rejected, ...raids, ...milestones, ...chapters];
    }

    case "Transactions":
    case "TransactionRecorded":
      return applyTransactions(st, msg) ? [{ kind: "TransactionsApplied" }] : [];

    case "Trade":
      return [{ kind: "TradeSettled", trade: msg.trade }];

    case "Error":
      return st.playerId === null
        ? [{ kind: "JoinRejected", message: msg.message }]
        : [{ kind: "ServerError", message: msg.message }];
  }
}

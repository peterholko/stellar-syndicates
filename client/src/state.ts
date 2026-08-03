// The client's view of the world: the latest per-player message the server has
// pushed. This is *not* authoritative — in M2 it is the TRUE world (movement
// verification); in M3 it becomes a delayed, fogged picture.

import type {
  EmplacementView, AnchorView, CharterView, FleetDoctrine, FreightView, GalaxyInfo, GhostView, MarketView, PathPointView, PendingOrderView, PlayerId, StandingOrder, SystemStateView, TimelineEntry, Vec2, WalletView } from "./protocol";
import { defaultDoctrine } from "./protocol";

export type LinkStatus = "connecting" | "online" | "offline";

const CLOCK_MAX_SLEW = 0.05; // Tunable: at most ±5% catch-up / fall-back.
const CLOCK_GAIN = 3.3; // Tunable: unconstrained errors converge in roughly 0.3 s.
const CLOCK_SNAP_S = 1.0; // Tunable: reconnect/restart/tab-wake discontinuity.

let renderClockReady = false;
let renderClockSim = 0;
let renderClockWallMs = 0;
let renderClockRate = 1;

/// The one continuous render clock. Snapping it to every 10 Hz View leaked
/// transport jitter straight into pixels: displacement = clock error × object
/// speed × zoom, most visibly on the deliberately un-eased order comet. Normal
/// View error is absorbed by a bounded positive slew, so this clock is monotonic
/// by construction. A >1 s discontinuity instead hard-snaps on purpose, covering
/// reconnects, server restarts, and a tab waking after a long suspension.
export function syncRenderClock(simTime: number, wallMs = performance.now()): void {
  if (!renderClockReady) {
    renderClockReady = true;
    renderClockSim = simTime;
    renderClockWallMs = wallMs;
    renderClockRate = 1;
    return;
  }

  const current = liveSimTime(wallMs);
  const error = simTime - current;
  renderClockSim = Math.abs(error) > CLOCK_SNAP_S ? simTime : current;
  renderClockWallMs = wallMs;
  renderClockRate = Math.abs(error) > CLOCK_SNAP_S
    ? 1
    : 1 + Math.max(-CLOCK_MAX_SLEW, Math.min(CLOCK_MAX_SLEW, error * CLOCK_GAIN));
}

export function liveSimTime(wallMs = performance.now()): number {
  if (!renderClockReady) return 0;
  const elapsed = Math.max(0, (wallMs - renderClockWallMs) / 1000);
  return renderClockSim + elapsed * renderClockRate;
}

/// §comms-v4: an own fleet gets the stationary tunnel presentation only when
/// the PLAYER'S served picture says both that it is beyond every comm bubble
/// and that its hyperdrive is still stirring the lane. Age never enters this
/// predicate: the arrived drive fact alone decides the presentation.
export function ghostInTunnel(ghost: GhostView): boolean {
  if (!ghost.own || ghost.in_comms) return false;
  const drive = ghost.drive;
  if (!drive || typeof drive === "string") return false;
  return ("cruising" in drive && drive.cruising === "hyperspace")
    || ("dropping" in drive && drive.dropping.from === "hyperspace");
}

// The OUTBOUND command signal: the violet comet of an order crossing space from
// the command center to the ship, over [depart, arrive]. Pure rendering — the
// client only interpolates between the server-provided times. There is no inbound
// "response" leg: the ship's reaction is seen directly on the map in delayed
// light, so animating a confirmation travelling home would just duplicate the map.
export interface CommandSignal {
  orderId: number;
  shipId: string;
  depart: number; // sim-time the order left the command center
  arrive: number; // sim-time it reaches the ship (observed)
  pOut: number; // 0..1 outbound progress, recomputed each frame
  /// §buoys: the relay path (hop positions + arrival fractions). The comet
  /// traces these — sprinting along lanes, crawling the gaps — instead of a
  /// straight line. Empty = no relays helped; fly direct.
  hops: { pos: Vec2; frac: number }[];
  beyondComms: boolean;
}

export type OrderVerb = "move" | "raid" | "attack" | "blockade" | "demolish" | "survey";

/// A map order the player is comparing but has not committed. This lives in
/// module state (not the 10 Hz rebuilt DOM), so replacing/cancelling an intent
/// always removes both its confirm affordance and its map preview together.
export interface PendingIntent {
  shipId: string;
  verb: OrderVerb;
  dest?: Vec2;
  targetId?: string;
  path?: PathPointView[];
}

// NOTE: a raid result has NO inbound travelling signal. The map IS the inbound
// sensor feed — when the destruction's light reaches the player, the doomed
// ghost vanishes on the map at the kill location (server-driven), and at that
// same observed moment a NOTIFICATION is logged. There is no second "news
// travelling home" channel (it would depict information that already arrived).

export interface ViewState {
  playerId: PlayerId | null;
  name: string;
  tickHz: number;
  tick: number;
  simTime: number;
  /// Distinct corporations the player can currently SEE (self + rivals whose
  /// light has reached them). Derived from ghosts — light-respecting, unlike a
  /// raw connection count.
  corpsInView: number;
  link: LinkStatus;

  // World view (delayed/fogged from the player's command center).
  galaxy: GalaxyInfo | null;
  commandCenter: Vec2 | null;
  anchors: AnchorView[];
  /// Per-tick dynamic system state (ownership light-gated, stockpile owner-only),
  /// keyed by system id, paired with the static `galaxy.systems` geology.
  systems: SystemStateView[];
  ghosts: GhostView[];
  /// §comms-v4.1: the first served point at which each own coupled fleet was
  /// seen beyond comms. Position, heading, emission clock, and known plan are
  /// frozen together: this is a stationary map bookmark, not a position feed.
  tunnelBookmarks: Record<string, {
    pos: Vec2;
    vel: Vec2;
    reportedAt: number;
    path: PathPointView[] | null;
  }>;
  /// §emplacements: your own structures standing in open space.
  emplacements: EmplacementView[];
  market: MarketView | null;
  /// Client-accumulated history of the (light-delayed) hub prices the player has
  /// OBSERVED, per commodity — the data source for the Market sparklines. This is
  /// fog-safe by construction: it stores only the lagged prices already shown in
  /// the ticker, never server truth. Sampled at ~1 Hz (see recordPriceHistory),
  /// capped to a rolling window. Newest sample last.
  priceHistory: Record<string, number[]>;
  /// Sim-time of the last price-history sample, to throttle accumulation.
  lastPriceSampleAt: number;
  wallet: WalletView | null;
  /// §TCA: the Market Hub freight desk — timetable, per-destination terms, and
  /// the player's OWN shipment queue. Owner-only, fresh from the View.
  freight: FreightView | null;
  /// §TCA Phase 2: the player's OWN charter standing and band.
  charter: CharterView | null;
  /// §perf Part B: the static charter band ladder — arrives once in Welcome.
  charterLadder: [string, number][];
  /// §perf Part B: the static research programme catalog — arrives once in
  /// Welcome; the View's dynamic research slice joins onto it by id.
  researchCatalog: import("./protocol").ProgrammeInfo[];
  /// The player's own standing logistics orders (§15), fresh from the View.
  standingOrders: StandingOrder[];
  /// The player's own fleet doctrine (§16), fresh from the View.
  doctrine: FleetDoctrine;
  /// §order-lifecycle: every own in-flight order, grouped by fleet and sorted
  /// oldest first. Drives the queue, comets, and map echo badge.
  pendingOrders: Map<string, PendingOrderView[]>;
  /// §battles-take-time: ongoing battles visible to this player (light-gated).
  battles: import("./protocol").BattleView[];
  /// §battle-aftermath: retained concluded battles this player was IN — present
  /// only after their conclusion light arrived (owner-only, from the View).
  battleReports: import("./protocol").BattleReportView[];
  /// §contestable-territory Part 2: retained captures this player participated in.
  captureReports: import("./protocol").CaptureReportView[];
  /// §battle-records Part A: the light-gated replay of every observable battle
  /// (running + recent), at the viewer's fidelity. Keyed use is by id.
  battleRecords: import("./protocol").BattleRecordView[];
  /// §ground G2: LANDING records, held and merged exactly like battle records.
  groundRecords: import("./protocol").GroundRecordView[];
  /// §syndicates Part 1: the viewer's OWN syndicate roster (null if unaffiliated).
  syndicate: import("./protocol").SyndicateView | null;
  /// §syndicates Part 1: pending invitations the viewer may accept.
  syndicateInvites: import("./protocol").SyndicateInviteView[];
  /// §rankings: the PUBLISHED leaderboard (public snapshot on the ledger close).
  rankings: import("./protocol").RankingRow[];
  /// §research R6: the viewer's OWN research picture (null if unaffiliated).
  research: import("./protocol").ResearchView | null;
  /// §battle-aftermath: report ids the player has OPENED (viewed → static/dim
  /// marker) and DISMISSED (marker hidden; the report stays in the log).
  /// Client-local, persisted to localStorage across reloads.
  battleViewed: Set<number>;
  battleDismissed: Set<number>;
  /// The check-in timeline (§16, Layer 3): what became observable, newest last.
  timeline: TimelineEntry[];
  /// Sim-time the player was last online — the "while you were away" boundary.
  /// Captured from the first Timeline message of the session.
  awaySince: number;
  /// Whether `awaySince` has been latched this session (so live re-sends don't
  /// move the boundary).
  awaySet: boolean;
  // Interaction.
  selectedShipId: string | null;
  /// Currently selected star system (for the claim / ship-production panel).
  selectedSystemId: string | null;
  /// §emplacements: the selected structure standing in open space (buoy or
  /// sensor). Shares the right-dock panel with ships — never both at once.
  selectedEmplacementId: string | null;
  /// Committed raids the player issued (raiderId → targetId), so the renderer can
  /// draw a CRUDE, drifting intercept estimate for each. Cleared on recall, on the
  /// result notification, or when either ship leaves the view.
  raids: Record<string, string>;
  /// Client-side record of move orders the player issued (shipId → destination),
  /// purely for drawing the "commanded into the dark" line. The server never
  /// echoes orders back (that's internal truth).
  orders: Record<string, Vec2>;
  /// Deliberate map-order preview. No command is sent until this is confirmed.
  pendingIntent: PendingIntent | null;
  /// Order queue selection is persistent module state, never DOM-only—the ship
  /// panel is rebuilt at view cadence. Rows are its only selection surface.
  selectedOrderId: number | null;

  // The OUTBOUND order/recall signal (command center → ship). Stays — it depicts
  // a real channel (your command crossing space) the player can't otherwise see.
  commandSignals: CommandSignal[];
}

export function initialState(): ViewState {
  return {
    playerId: null,
    name: "",
    tickHz: 30,
    tick: 0,
    simTime: 0,
    corpsInView: 0,
    link: "connecting",
    galaxy: null,
    commandCenter: null,
    anchors: [],
    systems: [],
    ghosts: [],
    tunnelBookmarks: {},
    emplacements: [],
    market: null,
    priceHistory: {},
    lastPriceSampleAt: -1,
    wallet: null,
    freight: null,
    charter: null,
    charterLadder: [],
    researchCatalog: [],
    standingOrders: [],
    doctrine: defaultDoctrine(),
    pendingOrders: new Map(),
    battles: [],
    battleReports: [],
    battleRecords: [],
    groundRecords: [],
    captureReports: [],
    syndicate: null,
    syndicateInvites: [],
    rankings: [],
    research: null,
    battleViewed: new Set(),
    battleDismissed: new Set(),
    timeline: [],
    awaySince: 0,
    awaySet: false,
    selectedShipId: null,
    selectedSystemId: null,
    selectedEmplacementId: null,
    raids: {},
    orders: {},
    pendingIntent: null,
    selectedOrderId: null,
    commandSignals: [],
  };
}

// The client's view of the world: the latest per-player message the server has
// pushed. This is *not* authoritative — in M2 it is the TRUE world (movement
// verification); in M3 it becomes a delayed, fogged picture.

import type { AnchorView, GalaxyInfo, GhostView, MarketView, PlayerId, RaidReport, SystemStateView, Vec2, WalletView } from "./protocol";

export type LinkStatus = "connecting" | "online" | "offline";

// Visualizations of communication delay the server owns the timing for. Both are
// pure rendering: the client only interpolates between server-provided endpoints
// and times. `progress` (0..1) is recomputed each frame.

// Order round-trip signal: comet out (command center → ghost) over
// [depart, arrive], then the response light home (ghost → command center) over
// [arrive, observe], when the ghost visibly changes course. Paced by sim-time.
export interface CommandSignal {
  shipId: string;
  depart: number; // sim-time the order left the command center
  arrive: number; // sim-time it reaches the ship (observed)
  observe: number; // sim-time the ship's response light reaches the command center
  // Recomputed each frame:
  phase: "out" | "back";
  pOut: number; // 0..1 outbound progress
  pBack: number; // 0..1 return progress
  remainingS: number; // seconds until the response is observable
}

// A ship the server has destroyed, kept flying as a ghost on the client (on old
// light) until its result ring reaches the command center — so it vanishes IN
// SYNC with the yellow signal arrival, not at the earlier moment the server first
// stops sending it. `ghost` is the snapshot taken when the report arrived;
// `capturedWallMs` lets us dead-reckon it onward.
export interface DoomedGhost {
  ghost: GhostView;
  capturedWallMs: number;
}

// Inbound result rings: resolution point → command center. Departs when the
// report becomes observable (server-gated, M4) and travels home over the
// server-provided light delay (`report.age`); the verdict is revealed on arrival.
// Any ship this report destroyed is carried in `doomed` and kept rendered until
// the ring lands (then it vanishes with the verdict — §6).
export interface ReportSignal {
  from: Vec2;
  startWallMs: number;
  durationS: number;
  report: RaidReport;
  progress: number;
  doomed: DoomedGhost[];
}

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
  market: MarketView | null;
  wallet: WalletView | null;
  /// Wall-clock ms when the last View arrived, for smooth extrapolation
  /// between the ~10 Hz server updates and the 60 fps render.
  lastViewWallMs: number;

  // Interaction.
  selectedShipId: string | null;
  /// Currently selected star system (for the claim / ship-production panel).
  selectedSystemId: string | null;
  /// Client-side record of move orders the player issued (shipId → destination),
  /// purely for drawing the "commanded into the dark" line. The server never
  /// echoes orders back (that's internal truth).
  orders: Record<string, Vec2>;

  // Traveling-signal visualizations (server-timed; client only interpolates).
  commandSignals: CommandSignal[];
  reportSignals: ReportSignal[];
  /// The most recent ghost seen for each ship id (with wall-time), so a report
  /// arriving the same tick the server drops the ghost can still capture it as a
  /// `DoomedGhost`. Pruned a few seconds after a ship was last seen.
  recentGhosts: Map<string, { ghost: GhostView; seenWallMs: number }>;
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
    market: null,
    wallet: null,
    lastViewWallMs: 0,
    selectedShipId: null,
    selectedSystemId: null,
    orders: {},
    commandSignals: [],
    reportSignals: [],
    recentGhosts: new Map(),
  };
}

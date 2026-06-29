// The client's view of the world: the latest per-player message the server has
// pushed. This is *not* authoritative — in M2 it is the TRUE world (movement
// verification); in M3 it becomes a delayed, fogged picture.

import type { AnchorView, FleetDoctrine, GalaxyInfo, GhostView, MarketView, PlayerId, StandingOrder, SystemStateView, TimelineEntry, Vec2, WalletView } from "./protocol";
import { defaultDoctrine } from "./protocol";

export type LinkStatus = "connecting" | "online" | "offline";

// The OUTBOUND command signal: the violet comet of an order crossing space from
// the command center to the ship, over [depart, arrive]. Pure rendering — the
// client only interpolates between the server-provided times. There is no inbound
// "response" leg: the ship's reaction is seen directly on the map in delayed
// light, so animating a confirmation travelling home would just duplicate the map.
export interface CommandSignal {
  shipId: string;
  depart: number; // sim-time the order left the command center
  arrive: number; // sim-time it reaches the ship (observed)
  pOut: number; // 0..1 outbound progress, recomputed each frame
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
  market: MarketView | null;
  wallet: WalletView | null;
  /// The player's own standing logistics orders (§15), fresh from the View.
  standingOrders: StandingOrder[];
  /// The player's own fleet doctrine (§16), fresh from the View.
  doctrine: FleetDoctrine;
  /// The check-in timeline (§16, Layer 3): what became observable, newest last.
  timeline: TimelineEntry[];
  /// Sim-time the player was last online — the "while you were away" boundary.
  /// Captured from the first Timeline message of the session.
  awaySince: number;
  /// Whether `awaySince` has been latched this session (so live re-sends don't
  /// move the boundary).
  awaySet: boolean;
  /// Wall-clock ms when the last View arrived, for smooth extrapolation
  /// between the ~10 Hz server updates and the 60 fps render.
  lastViewWallMs: number;

  // Interaction.
  selectedShipId: string | null;
  /// Currently selected star system (for the claim / ship-production panel).
  selectedSystemId: string | null;
  /// Committed raids the player issued (raiderId → targetId), so the renderer can
  /// draw a CRUDE, drifting intercept estimate for each. Cleared on recall, on the
  /// result notification, or when either ship leaves the view.
  raids: Record<string, string>;
  /// Client-side record of move orders the player issued (shipId → destination),
  /// purely for drawing the "commanded into the dark" line. The server never
  /// echoes orders back (that's internal truth).
  orders: Record<string, Vec2>;

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
    market: null,
    wallet: null,
    standingOrders: [],
    doctrine: defaultDoctrine(),
    timeline: [],
    awaySince: 0,
    awaySet: false,
    lastViewWallMs: 0,
    selectedShipId: null,
    selectedSystemId: null,
    raids: {},
    orders: {},
    commandSignals: [],
  };
}

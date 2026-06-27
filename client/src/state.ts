// The client's view of the world: the latest per-player message the server has
// pushed. This is *not* authoritative — in M2 it is the TRUE world (movement
// verification); in M3 it becomes a delayed, fogged picture.

import type { AnchorView, GalaxyInfo, GhostView, PlayerId, Vec2 } from "./protocol";

export type LinkStatus = "connecting" | "online" | "offline";

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
  ghosts: GhostView[];
  /// Wall-clock ms when the last View arrived, for smooth extrapolation
  /// between the ~10 Hz server updates and the 60 fps render.
  lastViewWallMs: number;

  // Interaction.
  selectedShipId: string | null;
  /// Client-side record of move orders the player issued (shipId → destination),
  /// purely for drawing the "commanded into the dark" line. The server never
  /// echoes orders back (that's internal truth).
  orders: Record<string, Vec2>;
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
    ghosts: [],
    lastViewWallMs: 0,
    selectedShipId: null,
    orders: {},
  };
}

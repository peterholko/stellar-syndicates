// The client's view of the world: the latest per-player message the server has
// pushed. This is *not* authoritative — in M2 it is the TRUE world (movement
// verification); in M3 it becomes a delayed, fogged picture.

import type { AnchorView, GalaxyInfo, PlayerId, ShipView } from "./protocol";

export type LinkStatus = "connecting" | "online" | "offline";

export interface ViewState {
  playerId: PlayerId | null;
  name: string;
  tickHz: number;
  tick: number;
  simTime: number;
  playersOnline: number;
  link: LinkStatus;

  // World view.
  galaxy: GalaxyInfo | null;
  anchors: AnchorView[];
  ships: ShipView[];
  /// Wall-clock ms when the last View arrived, for smooth extrapolation
  /// between the ~10 Hz server updates and the 60 fps render.
  lastViewWallMs: number;
}

export function initialState(): ViewState {
  return {
    playerId: null,
    name: "",
    tickHz: 30,
    tick: 0,
    simTime: 0,
    playersOnline: 0,
    link: "connecting",
    galaxy: null,
    anchors: [],
    ships: [],
    lastViewWallMs: 0,
  };
}

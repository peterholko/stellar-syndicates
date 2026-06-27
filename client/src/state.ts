// The client's view of the world: the latest per-player message the server has
// pushed. This is *not* authoritative — it is a delayed, fogged picture (fully
// realised in M3). For M1 it is just identity + the live clock.

import type { PlayerId } from "./protocol";

export type LinkStatus = "connecting" | "online" | "offline";

export interface ViewState {
  playerId: PlayerId | null;
  name: string;
  tickHz: number;
  tick: number;
  simTime: number;
  playersOnline: number;
  link: LinkStatus;
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
  };
}

// Live battle playback, shared by both shells: trail the newest arrived round
// so the replay never runs dry between arrivals.
//
// Rounds reach the client on the server's update beat (10 Hz desktop, 5 Hz
// phone), so each lands up to a beat late. A playhead that chased the newest
// round stopped whenever one did: the hulls froze for a few frames, then moved
// on. Live playback instead trails the live edge by one round plus
// LIVE_BUFFER_S, easing its speed (0.8–1.25×) to hold that. The edge is
// extrapolated from the newest arrival at the battle's pace, so the speed
// answers real lateness or drift, never the step each arrival makes.

/** Slack for a round that lands late, beyond the one round always in hand. */
export const LIVE_BUFFER_S = 0.25;
/** Further behind than this (a hidden tab, a burst of late rounds): jump back. */
const LIVE_SNAP_S = 3;
/** Playback speed change per second of lag error, and its bounds. */
const EASE_GAIN = 0.8;
const EASE_MIN = 0.8;
const EASE_MAX = 1.25;

export interface LivePlayhead {
  round: number;
  /** Progress through the window from `round` to the next (0–1). */
  frac: number;
}

type Rounds = readonly { tick: number }[];

/** Wall seconds per round at the live edge (the newest window's spacing at
 * `pace` sim seconds per wall second). */
export function liveRoundSeconds(rounds: Rounds, hz: number, pace: number): number {
  const n = rounds.length;
  const ticks = n >= 2 ? rounds[n - 1].tick - rounds[n - 2].tick : 15;
  return Math.max(0.05, ticks / Math.max(1, hz) / Math.max(1e-6, pace));
}

/** Where the live edge is now, in rounds: the newest arrived round carried
 * forward since it landed, up to when the next one is due. */
function liveEdge(frontier: number, sinceArrivalS: number, roundS: number): number {
  return frontier + Math.min(1, Math.max(0, sinceArrivalS) / roundS);
}

/** Rounds to trail the extrapolated edge by. */
function targetLag(roundS: number): number {
  return 1 + LIVE_BUFFER_S / roundS;
}

function at(position: number): LivePlayhead {
  const round = Math.floor(position);
  return { round, frac: position - round };
}

/** The playhead a live battle opens at. */
export function liveStart(rounds: Rounds, hz: number, pace: number, sinceArrivalS = 0): LivePlayhead {
  const frontier = rounds.length - 1;
  if (frontier < 0) return { round: 0, frac: 0 };
  const roundS = liveRoundSeconds(rounds, hz, pace);
  return at(Math.min(frontier, Math.max(0, liveEdge(frontier, sinceArrivalS, roundS) - targetLag(roundS))));
}

/** Advance a live playhead by `dt` wall seconds, never past the newest arrived
 * round. `final`: the battle is over and nothing more will arrive, so it plays
 * out to the end at the battle's own pace. */
export function advanceLive(head: LivePlayhead, rounds: Rounds, hz: number, pace: number,
  sinceArrivalS: number, dt: number, final: boolean): LivePlayhead {
  const frontier = rounds.length - 1;
  if (frontier < 0) return head;
  const roundS = liveRoundSeconds(rounds, hz, pace);
  const position = Math.min(frontier, head.round + head.frac);
  const edge = liveEdge(frontier, sinceArrivalS, roundS);
  const lagError = (edge - position - targetLag(roundS)) * roundS;
  if (!final && lagError > LIVE_SNAP_S) return at(Math.min(frontier, Math.max(0, edge - targetLag(roundS))));
  const ease = final ? 1 : Math.min(EASE_MAX, Math.max(EASE_MIN, 1 + lagError * EASE_GAIN));
  return at(Math.min(frontier, position + dt * ease / roundS));
}

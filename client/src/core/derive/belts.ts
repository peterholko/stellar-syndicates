import type { BodyView, Commodity, Deposit, GhostView, PlayerId, StationAnchor, SystemStateView } from "../../protocol";
import { fleetCargoManifest } from "../../protocol";
import { label } from "../../icons";
import { fleetCargoCapacity, mineCapable, prospectCapable, shipKindLabel } from "./fleet";

/** §belts: mirror of the sim's ANCHOR_SECS (anchoring, and unanchoring). */
export const ANCHOR_SECS = 60;

/** §belts: a lone Mining Belt Station (the only fleet that anchors). */
export const isStation = (g: Pick<GhostView, "kind" | "composition">): boolean =>
  g.composition ? g.composition.length === 1 && g.composition[0].kind === "mining_belt_station" && g.composition[0].count === 1
    : g.kind === "mining_belt_station";

/** §belts: a station's anchor in a few words: "Anchored", "Anchoring · 42 s". */
export function anchorStatus(anchor: StationAnchor | null | undefined, now: number): string {
  if (!anchor) return "Not anchored";
  const left = Math.max(1, Math.ceil(ANCHOR_SECS - (now - anchor.since)));
  return anchor.phase === "approaching" ? "En route" : anchor.phase === "anchoring" ? `Anchoring · ${left} s`
    : anchor.phase === "unanchoring" ? `Unanchoring · ${left} s` : "Anchored";
}

/** §belts: your station at a belt, with its hold. */
export interface BeltStation { fleet: string; anchor: StationAnchor; held: number; capacity: number }

/** One button's worth of belt work for one fleet. */
export interface BeltAction {
  fleet: string;
  text: string;
  ore?: Commodity;
  pocket?: boolean;
}

/** §belts/§pockets: what your fleets can do at a charted belt. A Scout or
 * barge prospects any charted belt; a barge mines its ores and the pockets
 * you have found, in an unowned system or one of yours. `note` says why
 * nothing can be mined. Shared by the desktop world panel and mobile. */
export function beltActions(ghosts: GhostView[], playerId: PlayerId | null, dynamic: SystemStateView, body: BodyView):
  { prospect: BeltAction[]; mine: BeltAction[]; anchor: BeltAction[]; station: BeltStation | null; note: string; yours: boolean } {
  const yours = dynamic.owner != null && dynamic.owner === playerId;
  const rival = dynamic.owner != null && !yours && !dynamic.ally;
  const own = ghosts.filter((g) => g.own && !g.tca);
  const barges = own.filter(mineCapable).slice(0, 3);
  const prospectors = own.filter(prospectCapable).slice(0, 3);
  const tag = (g: GhostView, many: boolean) => many ? ` · ${shipKindLabel(g.kind)} ${g.id}` : "";
  const ores = [...(body.deposits ?? []).map((d) => ({ ore: d.resource, pocket: false })),
    ...(body.pockets ?? []).filter((d) => (d.reserves ?? 1) > 0).map((d) => ({ ore: d.resource, pocket: true }))];
  const prospect = prospectors.map((g) => ({ fleet: g.id, text: `Prospect${tag(g, prospectors.length > 1)}` }));
  const mine = rival ? [] : barges.flatMap((g) => ores.map(({ ore, pocket }) =>
    ({ fleet: g.id, ore, pocket, text: `Mine ${label(ore)}${pocket ? " pocket" : ""}${tag(g, barges.length > 1)}` })));
  const note = rival ? "Another corporation holds this system."
    : !ores.length ? "Bare rock: nothing to mine."
    : !barges.length ? "Build a Mining Barge at a Shipyard to mine this belt." : "";
  // §belts: your station here, or the free stations that could anchor here
  // (unowned or allied systems only: a colony stockpiles its own ore).
  const here = own.find((g) => g.anchor?.system === dynamic.id && g.anchor.body === body.id);
  const station = here?.anchor ? { fleet: here.id, anchor: here.anchor,
    held: fleetCargoManifest(here).reduce((n, c) => n + c.units, 0), capacity: fleetCargoCapacity(here) } : null;
  const free = own.filter((g) => isStation(g) && (!g.anchor || g.anchor.phase === "approaching")).slice(0, 3);
  const anchor = station || yours || rival ? [] : free.map((g) => ({ fleet: g.id, text: `Anchor station${tag(g, free.length > 1)}` }));
  return { prospect, mine, anchor, station, note, yours };
}

/** §pockets: the belt's prospecting line: "Unprospected", or "Prospected 3× ·
 * 1 pocket found". Empty for anything but a surveyed belt. */
export function prospectLine(body: BodyView): string {
  if (body.kind !== "asteroid_belt" || body.deposits === null) return "";
  const found = body.pockets?.length ?? 0;
  if (!body.prospected) return found ? `${found} pocket${found > 1 ? "s" : ""} found` : "Unprospected";
  return `Prospected ${body.prospected}× · ${found ? `${found} pocket${found > 1 ? "s" : ""} found` : "no pockets found"}`;
}

/** §pockets: a pocket's reserves line. */
export const pocketLeft = (pocket: Deposit): string =>
  pocket.reserves === null ? "Renewable" : pocket.reserves <= 0 ? "Depleted" : `${Math.round(pocket.reserves).toLocaleString()} left`;

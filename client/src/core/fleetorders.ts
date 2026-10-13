import type { ClientMsg, GhostView } from "../protocol";
import type { PendingIntent, ViewState } from "../state";
import { label } from "../icons";
import { dockedAtSystem, slowCapable, estimatedFuelForLeg, regimeStore, fleetBaseSpeed, fleetCargoCapacity, guardCapable, mineCapable, prospectCapable, PROSPECT_SECS, shipKindLabel, transitLabel, transitSpeedMultiplier, WARP_FACTOR } from "./derive/fleet";
import { ANCHOR_SECS, isStation } from "./derive/belts";

/** §belts: mirror of the sim's DOCK_RADIUS: a freighter loads beside its station. */
const STATION_DOCK_SU = 260;
import { expeditionCapable, deepExpeditionReason, siteTitle } from "./derive/exploration";
import { fmtDur } from "./derive/format";
import { tenderCapable, tenderFuel } from "./derive/tenders";
import { FIELD_FUEL, FIELD_RADIUS_SU, FIELD_SECONDS, interdictionReason } from "./derive/interdiction";
import { freightRouteReason } from "./derive/industry";

/** Fleet controls stage the exact payload, never an optimistic order. Map
 * destinations and settings share the same Confirm/Cancel surface; only the
 * confirmation path may transmit. No callback reads a changed dropdown later. */
export type FleetCommand = Extract<ClientMsg, { type:
  | "MoveShip" | "JumpShip" | "HoldFleet" | "RecallRaid" | "Withdraw"
  | "CommitRaid" | "AttackFleet" | "GuardFleet" | "DefendSystem" | "BlockadeSystem" | "SurveySystem"
  | "SetFleetTransit" | "SetFleetPosture" | "SetFleetMission" | "SetEngageFreight" | "SetRunBlockades" | "SetFleetDoctrine"
  | "HubLoad" | "SystemLoad" | "HubUnload" | "SystemUnload"
  | "HaulToMarketHub" | "HaulToSystem" | "RequestFuelRescue" | "RefuelFleet"
  | "SplitFleet" | "MergeFleets" | "RefitShips" | "BuildEmplacement" | "DemolishEmplacement"
  | "AssignCaptain" | "ReserveCaptain" | "TrainCaptain"
  | "AssignOperationFleet" | "RecoverOperation"
  | "ExploreSite"
  | "SetFreightRoute" | "DeployOutpost" | "SkimFuel" | "MineBelt" | "ProspectBelt" | "DeployInterdiction"
  | "AnchorStation" | "UnanchorStation" | "StationLoad"
}>;

function recipient(command: FleetCommand): string | undefined {
  if ("fleet_id" in command) return command.fleet_id;
  if ("ship_id" in command) return command.ship_id;
  if ("raider_id" in command) return command.raider_id;
  if ("interceptor_id" in command) return command.interceptor_id;
  if ("builder" in command) return command.builder;
  if ("fleet" in command) return command.fleet;
  if (command.type === "MergeFleets") return command.into;
  return undefined;
}

/** Recheck received ownership/berths on Confirm. This is only a stale-control
 * guard: the sim still validates physical legality when the delayed order lands. */
export function fleetCommandsValid(commands: FleetCommand[], st: ViewState): boolean {
  if (!st.playerId || !commands.length) return false;
  const own = (id: string) => st.ghosts.find(g => g.own && g.id === id);
  return commands.every(command => {
    const id = recipient(command);
    const fleet = id ? own(id) : undefined;
    if (id && !fleet) return false;
    if (command.type === "SetFleetTransit" && command.mode === "slow") return !!fleet && slowCapable(fleet);
    if (command.type === "SetFreightRoute") return !!fleet && (!command.route || !freightRouteReason(st, fleet, command.route));
    if (command.type === "SkimFuel") return !!fleet && !fleet.tca
      && st.research?.programmes.some(p => p.id === "prop_expedition_v_ramscoop" && p.state === "completed") === true
      && st.systems.some(s => s.id === command.system_id && s.bodies.some(b => b.kind === "gas_giant"));
    // §belts: a barge, and a belt your corporation has charted holding that ore,
    // in an unowned system or one of yours.
    if (command.type === "MineBelt") return !!fleet && !fleet.tca && mineCapable(fleet)
      && st.systems.some(s => s.id === command.system_id && (s.owner == null || s.owner === st.playerId || !!s.ally)
        && s.bodies.some(b => b.id === command.body_id && b.kind === "asteroid_belt"
          && ((command.pocket ? b.pockets : b.deposits) ?? []).some(d => d.resource === command.ore)));
    // §pockets: a Scout or barge, and a belt your corporation has charted.
    if (command.type === "ProspectBelt") return !!fleet && !fleet.tca && prospectCapable(fleet)
      && st.systems.some(s => s.id === command.system_id
        && s.bodies.some(b => b.id === command.body_id && b.kind === "asteroid_belt"));
    // §belts: a lone station, a charted belt, in an unowned or allied system.
    if (command.type === "AnchorStation") return !!fleet && !fleet.tca && isStation(fleet)
      && (!fleet.anchor || fleet.anchor.phase === "approaching")
      && st.systems.some(s => s.id === command.system_id && (s.owner == null || (s.owner !== st.playerId && !!s.ally))
        && s.bodies.some(b => b.id === command.body_id && b.kind === "asteroid_belt"));
    if (command.type === "UnanchorStation") return !!fleet?.anchor && fleet.anchor.phase !== "unanchoring";
    // §belts: an idle freighter of yours beside your station.
    if (command.type === "StationLoad") {
      const station = own(command.station_id);
      return !!fleet && !!station && fleet.id !== station.id && fleetCargoCapacity(fleet) > 0 && !isStation(fleet)
        && Math.hypot(fleet.pos.x - station.pos.x, fleet.pos.y - station.pos.y) <= STATION_DOCK_SU && command.units > 0;
    }
    if (command.type === "DeployOutpost") return !!fleet && st.systems.some(s => s.id === command.system_id
      && s.owner === null && !s.bodies.some(b => b.habitable) && s.bodies.some(b => b.id === command.body_id && b.deposits !== null));
    if (command.type === "MergeFleets" && (!own(command.from) || command.from === command.into)) return false;
    if (command.type === "GuardFleet" && (!own(command.target_id) || command.target_id === id)) return false;
    if (command.type === "DeployInterdiction") return !!fleet && !interdictionReason(fleet, st);
    if (command.type === "RefuelFleet") return !!fleet && tenderCapable(fleet) && tenderFuel(fleet) > 0
      && !!own(command.target_id) && !own(command.target_id)?.tca && command.target_id !== id;
    if (command.type === "DefendSystem") return !!fleet && guardCapable(fleet)
      && st.systems.some(s => s.id === command.system_id && s.owner === st.playerId)
      && Number.isFinite(command.pursuit_radius) && command.pursuit_radius >= 5_000 && command.pursuit_radius <= 20_000;
    if (command.type === "Withdraw") return st.battles.some(b => b.own && b.participants.includes(id!)
      && !st.battleRecords.some(r => r.id === b.id && r.outcome !== null));
    if (command.type === "ExploreSite") {
      const site = st.explorationSites.find(s => s.id === command.site_id);
      return !!site && !!fleet && expeditionCapable(fleet, command.task)
        && (command.task === "investigate" || !!site.details)
        && (!['study', 'extract'].includes(command.task) || !deepExpeditionReason(site, fleet));
    }
    if (command.type === "HubLoad" || command.type === "HubUnload") return fleet?.docked === "hub";
    // Match the cargo panel: served berths use E29 while command IDs use 29.
    if (command.type === "SystemLoad" || command.type === "SystemUnload") return !!fleet && dockedAtSystem(fleet, command.system);
    if (command.type === "AssignCaptain" || command.type === "ReserveCaptain" || command.type === "TrainCaptain") {
      return st.captains.some(c => c.id === command.captain_id);
    }
    return true;
  });
}

export function fleetCommandIntent(input: FleetCommand | FleetCommand[], st: ViewState): PendingIntent | null {
  const commands = structuredClone(Array.isArray(input) ? input : [input]);
  if (!fleetCommandsValid(commands, st)) return null;
  const first = commands[0];
  const shipId = recipient(first) ?? "";
  const fleet = st.ghosts.find(g => g.own && g.id === shipId);
  // Clicking the already-served setting is not a new order.
  if (commands.length === 1 && fleet && (
    (first.type === "SetFleetTransit" && (fleet.transit ?? "full") === first.mode)
    || (first.type === "SetFleetPosture" && (fleet.posture ?? "passive") === first.posture)
    || (first.type === "SetFleetMission" && Object.entries(first.mission).every(([key, value]) =>
      (fleet.mission_profile ?? { priority: "balanced", screening: "automatic", withdrawal: "never" })[key as keyof typeof first.mission] === value))
  )) return null;
  const move = commands.find(c => c.type === "MoveShip");
  const defend = commands.find(c => c.type === "DefendSystem");
  const expedition = commands.find(c => c.type === "ExploreSite");
  const mine = commands.find(c => c.type === "MineBelt" || c.type === "ProspectBelt" || c.type === "AnchorStation");
  const survey = commands.find(c => c.type === "SurveySystem");
  const systemPos = (id: string) => st.galaxy?.systems.find(s => s.id === id)?.pos;
  return { verb: "command", shipId, commands, commander: st.playerId,
    dest: move?.dest ?? (expedition ? st.explorationSites.find(s => s.id === expedition.site_id)?.pos : undefined)
      ?? (defend ? systemPos(defend.system_id) : undefined) ?? (mine ? systemPos(mine.system_id) : undefined)
      ?? (survey ? systemPos(survey.system_id) : undefined) };
}

/** §founding R5: every order preview names where it goes. A point on a known
 * contact, system or the Hub reads by name; open space keeps coordinates. */
export function destinationLabel(dest: { x: number; y: number }, st: ViewState): string {
  const near = (p: { x: number; y: number }, r: number) => Math.hypot(p.x - dest.x, p.y - dest.y) <= r;
  const site = st.explorationSites.find(s => near(s.pos, 1_500));
  if (site) return siteTitle(site);
  const system = st.galaxy?.systems.find(s => near(s.pos, 1_500));
  if (system) return system.name;
  if (st.galaxy?.hub && near(st.galaxy.hub, 1_500)) return "Market Hub";
  return `${Math.round(dest.x)}, ${Math.round(dest.y)} su`;
}

/** Flight time and fuel for one leg, in the map-move preview's grammar. */
export function legEstimate(fleet: GhostView, dest: { x: number; y: number }): string {
  const distance = Math.hypot(dest.x - fleet.pos.x, dest.y - fleet.pos.y);
  const flight = distance / Math.max(1, fleetBaseSpeed(fleet) * WARP_FACTOR * transitSpeedMultiplier(fleet));
  const fuel = estimatedFuelForLeg(fleet, dest);
  // §fuel-overhaul: the store the fleet's throttle burns (Uranium on Slow).
  const store = regimeStore(fleet), served = store.fuel === "uranium" ? fleet.uranium : fleet.fuel;
  const tank = served == null ? "" : ` / ${Math.round(served)} aboard`;
  const short = served != null && fuel > served + 1e-6 ? " · FUEL SHORT" : "";
  return `${Math.round(distance).toLocaleString()} su · ETA ~${fmtDur((fleet.age ?? 0) + flight)} · ${label(store.fuel)} ~${Math.ceil(fuel)}${tank}${short}`;
}

function commandLabel(command: FleetCommand, st: ViewState): string {
  const system = (id: string) => st.galaxy?.systems.find(s => s.id === id)?.name ?? id;
  switch (command.type) {
    case "SetFleetTransit": return `Transit → ${transitLabel(command.mode)}${command.mode === "slow" ? " · ½ speed · Uranium" : ""}`;
    case "SetFreightRoute": return command.route ? `Route: ${command.route.name} · ${command.route.stops.length} stops · ${transitLabel(command.route.transit)} · ${command.route.fuel_reserve} ${command.route.transit === "slow" ? "Uranium" : "Deuterium"} reserve${command.route.shopping_budget == null ? "" : ` · buy missing return goods: max ${command.route.shopping_budget} cr per Hub visit (fees included); skip unaffordable goods`}` : "Stop freight route";
    case "DeployOutpost": return `Establish ${command.outpost} outpost → ${system(command.system_id)}`;
    case "SkimFuel": return `Harvest gas-giant Helium-3 → ${system(command.system_id)}`;
    case "MineBelt": {
      const belt = st.systems.find(s => s.id === command.system_id)?.bodies.find(b => b.id === command.body_id);
      return `Mine ${label(command.ore)}${command.pocket ? " pocket" : ""} → ${belt?.name ?? system(command.system_id)} · fills the barge's hold${st.systems.some(s => s.id === command.system_id && s.owner === st.playerId) ? ", or your stockpile here" : ""}`;
    }
    case "ProspectBelt": {
      const belt = st.systems.find(s => s.id === command.system_id)?.bodies.find(b => b.id === command.body_id);
      return `Prospect ${belt?.name ?? system(command.system_id)} · ${PROSPECT_SECS} s loud pass`;
    }
    case "AnchorStation": {
      const belt = st.systems.find(s => s.id === command.system_id)?.bodies.find(b => b.id === command.body_id);
      return `Anchor at ${belt?.name ?? system(command.system_id)} · ${ANCHOR_SECS} s on arrival`;
    }
    case "UnanchorStation": return `Unanchor · ${ANCHOR_SECS} s`;
    case "StationLoad": return `Load ${command.units} ${label(command.commodity)} from the station`;
    case "SetFleetPosture": return `Posture → ${label(command.posture)}`;
    case "SetFleetMission": return `Mission: ${label(command.mission.priority)} · ${label(command.mission.screening)} · ${command.mission.withdrawal === "never" ? "no damage retreat" : `withdraw below ${command.mission.withdrawal.slice(4)}% hull`}`;
    case "SetEngageFreight": return command.on ? "Engage Authority freight" : "Stop engaging Authority freight";
    case "SetRunBlockades": return command.on ? "Run held blockades" : "Wait held blockades out";
    case "SetFleetDoctrine": {
      const changes = Object.entries(command.doctrine).filter(([key, value]) => st.doctrine[key as keyof typeof st.doctrine] !== value);
      return `Fleet doctrine: ${changes.map(([key, value]) => `${label(key)} → ${label(String(value))}`).join(" · ") || "keep current settings"}`;
    }
    case "HoldFleet": return "Hold position";
    case "RecallRaid": return "Recall raid";
    case "Withdraw": return "Withdraw from battle";
    case "HubLoad": case "SystemLoad": return `Load ${command.units} ${label(command.commodity)}`;
    case "HubUnload": return "Unload cargo → Market Warehouse";
    case "SystemUnload": return `Unload cargo → ${system(command.system)}`;
    case "HaulToMarketHub": return `Haul → Market Hub${command.sell_on_arrival ? " · sell on arrival" : " · unload on arrival"}`;
    case "HaulToSystem": return `Haul → ${system(command.system)}`;
    case "RequestFuelRescue": return "Call AAA fuel rescue";
    case "RefuelFleet": return `Refuel ${shipKindLabel(st.ghosts.find(g => g.id === command.target_id)?.kind ?? "raider")} · ${command.target_id}`;
    case "DeployInterdiction": return `Raise an interdiction field here · ${FIELD_RADIUS_SU.toLocaleString()} su · up to ${FIELD_SECONDS / 60} min · ${FIELD_FUEL} Fuel · the fleet holds station`;
    case "SplitFleet": return `Split ${Object.entries(command.counts).map(([kind, n]) => `${n}× ${shipKindLabel(kind as GhostView["kind"])}`).join(", ")}`;
    case "MergeFleets": return `Merge fleet ${command.from} into ${command.into}`;
    case "RefitShips": {
      const g = st.ghosts.find(g => g.id === command.fleet_id);
      const dock = st.systems.find(s => g && dockedAtSystem(g,s.id));
      const yard = dock?.assignments.some(a => a.structure === "shipyard" && a.workers > 0);
      const berth = dock?.bodies.some(b => (b.structures.service_berth ?? 0) > 0);
      const borrowing = berth && !yard && !dock?.assignments.some(a => a.structure === "service_berth" && a.workers > 0);
      return `Refit ${command.n}× ${shipKindLabel(command.ship)} → ${command.to.map(label).join(", ") || "unfitted"}${borrowing ? " · Service Berth borrows one mining crew until fitting finishes; mining pauses/reduces, then resumes" : ""}`;
    }
    case "BuildEmplacement": return `Build ${label(command.emplacement)} here`;
    case "DemolishEmplacement": return `Demolish installation ${command.target}`;
    case "MoveShip": case "JumpShip": return `${command.type === "JumpShip" ? "Jump" : "Move"} → ${destinationLabel(command.dest, st)}`;
    case "GuardFleet": {
      const target = st.ghosts.find(g => g.id === command.target_id);
      return `Guard ${target ? shipKindLabel(target.kind) : "fleet"} · ${command.target_id}`;
    }
    case "AttackFleet": return `Attack fleet ${command.target_id}`;
    case "DefendSystem": return `Defend ${system(command.system_id)} · pursuit limit ${command.pursuit_radius.toLocaleString()} su`;
    case "CommitRaid": return `Raid fleet ${command.target_id}`;
    case "BlockadeSystem": return `Blockade ${system(command.system_id)}`;
    case "SurveySystem": return `Survey ${system(command.system_id)}`;
    case "ExploreSite": return `${({ investigate: "Investigate", restore: "Restore", recover: "Recover", study: "Deep investigation", extract: "Prepared extraction" })[command.task]} → ${st.explorationSites.find(s => s.id === command.site_id)?.details?.name ?? "Unknown contact"}`;
    case "AssignCaptain": return `Assign officer #${command.captain_id}`;
    case "ReserveCaptain": return `Return officer #${command.captain_id} to reserve`;
    case "TrainCaptain": return `Train officer #${command.captain_id}: ${label(command.attribute)}`;
    case "AssignOperationFleet": return `Assign to ${st.operations.find(o => o.id === command.operation_id)?.briefing?.title ?? "operation"}`;
    case "RecoverOperation": return "Recover operation salvage";
  }
}

export function fleetCommandSummary(intent: PendingIntent, st: ViewState): string {
  const fleet = st.ghosts.find(g => g.own && g.id === intent.shipId);
  const commands = intent.commands ?? [];
  const text = commands.map(c => commandLabel(c, st)).join("; ");
  if (!fleet) return text;
  const c = st.galaxy?.c;
  const cc = st.commandCenter;
  const delay = c && cc ? Math.hypot(fleet.pos.x - cc.x, fleet.pos.y - cc.y) / (c * WARP_FACTOR) : null;
  const move = commands.find(command => command.type === "MoveShip" && (command.ship_id === fleet.id));
  const mine = commands.find(command => command.type === "MineBelt" || command.type === "ProspectBelt" || command.type === "AnchorStation");
  const minePos = mine?.type === "MineBelt" || mine?.type === "ProspectBelt" || mine?.type === "AnchorStation"
    ? st.galaxy?.systems.find(s => s.id === mine.system_id)?.pos : undefined;
  const leg = move?.type === "MoveShip" ? ` · ${legEstimate(fleet, move.dest)}` : minePos ? ` · ${legEstimate(fleet, minePos)}` : "";
  return `${shipKindLabel(fleet.kind)} · ${text}${leg} · ${delay === null ? "command delay unknown" : `command ~${Math.ceil(delay)}s`}`;
}

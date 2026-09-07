import type { ClientMsg, GhostView } from "../protocol";
import type { PendingIntent, ViewState } from "../state";
import { label } from "../icons";
import { shipKindLabel, WARP_FACTOR } from "./derive/fleet";

/** Fleet controls stage the exact payload, never an optimistic order. Map
 * destinations and settings share the same Confirm/Cancel surface; only the
 * confirmation path may transmit. No callback reads a changed dropdown later. */
export type FleetCommand = Extract<ClientMsg, { type:
  | "MoveShip" | "JumpShip" | "HoldFleet" | "RecallRaid" | "Withdraw"
  | "CommitRaid" | "AttackFleet" | "GuardFleet" | "BlockadeSystem" | "SurveySystem"
  | "SetFleetTransit" | "SetFleetPosture" | "SetEngageFreight" | "SetFleetDoctrine"
  | "HubLoad" | "SystemLoad" | "HubUnload" | "SystemUnload"
  | "HaulToMarketHub" | "HaulToSystem" | "RequestFuelRescue"
  | "SplitFleet" | "MergeFleets" | "RefitShips" | "BuildEmplacement" | "DemolishEmplacement"
  | "AssignCaptain" | "ReserveCaptain" | "TrainCaptain"
  | "AssignOperationFleet" | "RecoverOperation"
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
    if (command.type === "MergeFleets" && (!own(command.from) || command.from === command.into)) return false;
    if (command.type === "GuardFleet" && (!own(command.target_id) || command.target_id === id)) return false;
    if (command.type === "Withdraw") return st.battles.some(b => b.own && b.participants.includes(id!)
      && !st.battleRecords.some(r => r.id === b.id && r.outcome !== null));
    if (command.type === "HubLoad" || command.type === "HubUnload") return fleet?.docked === "hub";
    if (command.type === "SystemLoad" || command.type === "SystemUnload") return fleet?.docked === command.system;
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
  )) return null;
  const move = commands.find(c => c.type === "MoveShip");
  return { verb: "command", shipId, commands, commander: st.playerId, dest: move?.dest };
}

function commandLabel(command: FleetCommand, st: ViewState): string {
  const system = (id: string) => st.galaxy?.systems.find(s => s.id === id)?.name ?? id;
  switch (command.type) {
    case "SetFleetTransit": return `Transit → ${command.mode === "stealth" ? "Stealth" : "Full speed"}`;
    case "SetFleetPosture": return `Posture → ${label(command.posture)}`;
    case "SetEngageFreight": return command.on ? "Engage Authority freight" : "Stop engaging Authority freight";
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
    case "SplitFleet": return `Split ${Object.entries(command.counts).map(([kind, n]) => `${n}× ${shipKindLabel(kind as GhostView["kind"])}`).join(", ")}`;
    case "MergeFleets": return `Merge fleet ${command.from} into ${command.into}`;
    case "RefitShips": return `Refit ${command.n}× ${shipKindLabel(command.ship)} → ${command.to.map(label).join(", ") || "unfitted"}`;
    case "BuildEmplacement": return `Build ${label(command.emplacement)} here`;
    case "DemolishEmplacement": return `Demolish installation ${command.target}`;
    case "MoveShip": case "JumpShip": return `${command.type === "JumpShip" ? "Jump" : "Move"} → ${Math.round(command.dest.x)}, ${Math.round(command.dest.y)} su`;
    case "GuardFleet": {
      const target = st.ghosts.find(g => g.id === command.target_id);
      return `Guard ${target ? shipKindLabel(target.kind) : "fleet"} · ${command.target_id}`;
    }
    case "AttackFleet": return `Attack fleet ${command.target_id}`;
    case "CommitRaid": return `Raid fleet ${command.target_id}`;
    case "BlockadeSystem": return `Blockade ${system(command.system_id)}`;
    case "SurveySystem": return `Survey ${system(command.system_id)}`;
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
  return `${shipKindLabel(fleet.kind)} · ${text} · ${delay === null ? "command delay unknown" : `command ~${Math.ceil(delay)}s`}`;
}

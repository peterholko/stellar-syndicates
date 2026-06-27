// Wire protocol types — mirror the server's `protocol.rs`. The client holds no
// authoritative state; these messages are the entire contract (§14).

// 64-bit id; sent as a decimal string to preserve precision beyond 2^53.
export type PlayerId = string;
export type EntityId = string;

export interface Vec2 {
  x: number;
  y: number;
}

export type ShipKind = "convoy" | "raider";

export interface SystemView {
  id: EntityId;
  pos: Vec2;
  name: string;
}

export interface GalaxyInfo {
  hub: Vec2;
  radius: number;
  c: number; // speed of light, sim units / s
  systems: SystemView[];
}

export interface AnchorView {
  pos: Vec2;
  owner: PlayerId | null;
}

// A ship as the player perceives it — a delayed "ghost" (§6). `pos` is where
// the object was when its arriving light left it; `age` is how stale that is;
// `uncertainty` is how far it could have moved since (0 for own ships).
export interface GhostView {
  id: EntityId;
  owner: PlayerId;
  kind: ShipKind;
  pos: Vec2;
  vel: Vec2;
  age: number;
  uncertainty: number;
  own: boolean;
}

// Render a decimal-string PlayerId as the canonical "P<hex>" form used by the
// server's Display impl. BigInt keeps the full 64 bits.
export function formatId(id: PlayerId): string {
  try {
    return "P" + BigInt(id).toString(16).padStart(16, "0");
  } catch {
    return id;
  }
}

// Client → server.
export type ClientMsg =
  | { type: "Join"; name: string }
  | { type: "MoveShip"; ship_id: EntityId; dest: Vec2 }
  | { type: "CommitRaid"; raider_id: EntityId; target_id: EntityId }
  | { type: "RecallRaid"; raider_id: EntityId }
  | { type: "Ping" };

export type RaidOutcome = "intercepted" | "escaped";

// A delayed raid report (§8) — arrives on the recipient's own clock.
export interface RaidReport {
  outcome: RaidOutcome;
  attacker: PlayerId;
  defender: PlayerId;
  raider: EntityId;
  convoy: EntityId;
  pos: Vec2;
  at_time: number;
  age: number; // light delay — how stale this news is
  you: "attacker" | "defender";
}

// Server → client.
export type ServerMsg =
  | {
      type: "Welcome";
      player_id: PlayerId;
      name: string;
      tick_hz: number;
      tick: number;
      sim_time: number;
      galaxy: GalaxyInfo;
    }
  | {
      type: "View";
      tick: number;
      sim_time: number;
      command_center: Vec2;
      anchors: AnchorView[];
      ghosts: GhostView[];
    }
  | { type: "Report"; report: RaidReport }
  | {
      // Outbound order feedback: a comet crossing from the command center to one
      // of your ships. The server owns the timing; the client only interpolates.
      type: "CommandSignal";
      ship_id: EntityId;
      depart_time: number;
      arrive_time: number;
    }
  | { type: "Error"; message: string };

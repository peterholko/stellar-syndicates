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

export interface ShipView {
  id: EntityId;
  owner: PlayerId;
  kind: ShipKind;
  pos: Vec2;
  vel: Vec2;
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
  | { type: "Ping" };

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
      players_online: number;
      anchors: AnchorView[];
      ships: ShipView[];
    }
  | { type: "Error"; message: string };

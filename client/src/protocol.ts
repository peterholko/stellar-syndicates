// Wire protocol types — mirror the server's `protocol.rs`. The client holds no
// authoritative state; these messages are the entire contract (§14).

// 64-bit id; sent as a decimal string to preserve precision beyond 2^53.
export type PlayerId = string;

// Client → server.
export type ClientMsg =
  | { type: "Join"; name: string }
  | { type: "Ping" };

// Render a decimal-string PlayerId as the canonical "P<hex>" form used by the
// server's Display impl. BigInt keeps the full 64 bits.
export function formatId(id: PlayerId): string {
  try {
    return "P" + BigInt(id).toString(16).padStart(16, "0");
  } catch {
    return id;
  }
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
    }
  | {
      type: "Tick";
      tick: number;
      sim_time: number;
      players_online: number;
    }
  | { type: "Error"; message: string };

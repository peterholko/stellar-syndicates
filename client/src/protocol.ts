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
  sensor_range: number; // detection radius each of your assets projects
  systems: SystemView[];
}

export type Commodity = "fuel" | "ore" | "alloys" | "provisions" | "volatiles";

export interface CargoView {
  commodity: Commodity;
  units: number;
}

export interface PriceView {
  commodity: Commodity;
  price: number;
}

// The hub ticker, light-delayed from the hub (§9). `staleness` = how old.
export interface MarketView {
  prices: PriceView[];
  staleness: number;
}

export interface InvSlot {
  commodity: Commodity;
  units: number;
}

export type Side = "buy" | "sell";

export interface OrderView {
  id: number;
  side: Side;
  commodity: Commodity;
  units: number;
  limit_price: number;
}

export interface WalletView {
  credits: number;
  valuation: number; // equity / net worth (slow §9 close)
  inventory: InvSlot[];
  orders: OrderView[];
}

// Economy news (mirrors sim TradeEvent, tagged by `event`).
export type TradeEvent =
  | { event: "Bought"; player: PlayerId; commodity: Commodity; units: number; unit_price: number }
  | { event: "Delivered"; player: PlayerId; commodity: Commodity; units: number }
  | { event: "SellDispatched"; player: PlayerId; commodity: Commodity; units: number }
  | { event: "Sold"; player: PlayerId; commodity: Commodity; units: number; unit_price: number }
  | { event: "LimitPlaced"; player: PlayerId; side: Side; commodity: Commodity; units: number; limit_price: number }
  | { event: "LimitFilled"; player: PlayerId; side: Side; commodity: Commodity; units: number; unit_price: number };

export interface AnchorView {
  pos: Vec2;
  owner: PlayerId | null;
}

// A ship as the player perceives it — a delayed "ghost" (§6). `pos` is where
// the object was when its arriving light left it; `age` is how stale that is;
// `uncertainty` is how far it could have moved since (`age × max_speed`). This
// holds for OWN ships too — there is no FTL tether to your fleet, so a distant
// own ship is as uncertain as a distant enemy; `own` is only a "this is mine"
// marker, never a certainty grant.
export interface GhostView {
  id: EntityId;
  owner: PlayerId;
  kind: ShipKind;
  pos: Vec2;
  vel: Vec2;
  age: number;
  uncertainty: number;
  own: boolean;
  // Convoys broadcast a route (waypoints); raiders don't (null).
  route: Vec2[] | null;
  // Cargo present only when this convoy is within your sensor coverage.
  cargo: CargoView | null;
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
  | { type: "MarketBuy"; commodity: Commodity; units: number }
  | { type: "MarketSell"; commodity: Commodity; units: number }
  | { type: "PlaceLimitOrder"; side: Side; commodity: Commodity; units: number; limit_price: number }
  | { type: "Ping" };

export type RaidOutcome =
  | "target_destroyed"
  | "attacker_destroyed"
  | "both_destroyed"
  | "both_survive"
  | "escaped";

// A delayed battle report (§8) — arrives on the recipient's own clock. One true
// outcome (seeded); both sides observe the same result, light-delayed.
export interface RaidReport {
  outcome: RaidOutcome;
  attacker: PlayerId;
  defender: PlayerId;
  attacker_ship: EntityId;
  target_ship: EntityId;
  attacker_kind: ShipKind;
  target_kind: ShipKind;
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
      market: MarketView;
      wallet: WalletView;
    }
  | { type: "Report"; report: RaidReport }
  | { type: "Trade"; trade: TradeEvent }
  | {
      // Order round-trip feedback: comet out (command center → ship), then the
      // response light coming home (ship → command center). The server owns all
      // three clock-times; the client only interpolates.
      type: "CommandSignal";
      ship_id: EntityId;
      depart_time: number;
      arrive_time: number;
      observe_time: number;
    }
  | { type: "Error"; message: string };

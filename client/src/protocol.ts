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

// A resource deposit on a system (static geology, public). Richer/more valuable
// toward the frontier — the distance/value gradient (§4).
export interface Deposit {
  resource: Commodity;
  richness: number; // units/sec at full extraction
  reserves: number | null; // null = renewable
}

// Static system geography + geology, sent once at join. Dynamic ownership/
// stockpile arrives light-gated per tick in `SystemStateView`.
export interface SystemInfo {
  id: EntityId;
  pos: Vec2;
  name: string;
  deposits: Deposit[];
  claim_cost: number;
}

// One commodity in a system's stockpile (whole units), shown only to its owner.
export interface StockSlot {
  commodity: Commodity;
  units: number;
}

// Per-tick, light-gated dynamic state of a system. `owner` is null until a
// rival's claim light arrives (own claims are instant); `stockpile` is present
// only for the owner — a rival's holdings never leak.
export interface SystemStateView {
  id: EntityId;
  owner: PlayerId | null;
  stockpile: StockSlot[] | null;
}

export interface GalaxyInfo {
  hub: Vec2;
  radius: number;
  c: number; // speed of light, sim units / s
  sensor_range: number; // detection radius each of your assets projects
  raider_speed: number; // raider cruise speed — for the crude intercept estimate
  systems: SystemInfo[];
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
  | { event: "LimitFilled"; player: PlayerId; side: Side; commodity: Commodity; units: number; unit_price: number }
  | { event: "AutoDispatched"; player: PlayerId; commodity: Commodity; units: number; source: EntityId; rule_id: number };

// --- Standing logistics orders (§15) — mirror the sim's serde shape exactly, so
// the client both reads them (from the View) and writes them (SetStandingOrder). ---
export type StandingEndpoint =
  | { kind: "system"; id: EntityId }
  | { kind: "hub" }
  | { kind: "home" };
export type StandingTrigger =
  | { kind: "above_threshold"; threshold: number }
  | { kind: "percent_surplus"; percent: number; floor: number }
  | { kind: "maintain_at_dest"; target: number };
export type OrderStatus = "active" | "paused";
export interface StandingOrder {
  id: number; // 0 on create = "allocate a fresh id"
  source: StandingEndpoint;
  dest: StandingEndpoint;
  commodity: Commodity;
  trigger: StandingTrigger;
  status: OrderStatus;
  next_eval_tick: number;
  in_flight: EntityId | null;
}

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
  | { type: "ClaimSystem"; system_id: EntityId }
  | { type: "ShipProduction"; system_id: EntityId }
  | { type: "SetStandingOrder"; order: StandingOrder }
  | { type: "ClearStandingOrder"; order_id: number }
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
      systems: SystemStateView[];
      ghosts: GhostView[];
      market: MarketView;
      wallet: WalletView;
      standing_orders: StandingOrder[];
    }
  | { type: "Report"; report: RaidReport }
  | { type: "Trade"; trade: TradeEvent }
  | {
      // OUTBOUND order feedback: the violet comet, command center → ship, over
      // [depart, arrive]. The server owns the clock-times; the client interpolates.
      // (No return leg — the ship's reaction is seen directly on the map.)
      type: "CommandSignal";
      ship_id: EntityId;
      depart_time: number;
      arrive_time: number;
    }
  | { type: "Error"; message: string };

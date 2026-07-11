// Wire protocol types — mirror the server's `protocol.rs`. The client holds no
// authoritative state; these messages are the entire contract (§14).

// 64-bit id; sent as a decimal string to preserve precision beyond 2^53.
export type PlayerId = string;
export type EntityId = string;

export interface Vec2 {
  x: number;
  y: number;
}

export type ShipKind = "convoy" | "raider" | "corvette" | "colony" | "scout";

// A resource deposit on a system. §explore: NO LONGER public — the exact geology
// is CORP KNOWLEDGE (surveyed-or-owner), delivered per-player in
// `SystemStateView.deposits`; the public spectral read is the `band` below.
export interface Deposit {
  resource: Commodity;
  richness: number; // units/sec at full extraction
  reserves: number | null; // null = renewable
}

/// §explore R1: the public richness band — the free spectral read (galaxy-wide
/// terciles; static, same for everyone).
export type RichnessBand = "poor" | "fair" | "rich";

// Static PUBLIC system geography, sent once at join. The exact geology is
// per-corp knowledge in `SystemStateView.deposits`; dynamic ownership/stockpile
// arrives light-gated per tick there too.
export interface SystemInfo {
  id: EntityId;
  pos: Vec2;
  name: string;
  /// §explore: the public richness band (Poor / Fair / Rich).
  band: RichnessBand;
  /// DEPRECATED (§ships part 3): claiming is physical (colony ships) — no
  /// longer charged or displayed; kept for wire compatibility.
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
// An owner-only in-progress build at a system (§step1). `key` = what's building
// ("convoy"|"raider"|"extractor"); `complete_time` = sim-time of completion.
export interface BuildState {
  key: string;
  complete_time: number;
}

export interface SystemStateView {
  id: EntityId;
  owner: PlayerId | null;
  stockpile: StockSlot[] | null;
  /// Owner-only in-progress build (§step1) — null for rivals (never leaks).
  /// The SOONEST job; `builds` carries the full queue.
  build: BuildState | null;
  /// Owner-only FULL build queue, completion-ordered (§build-progress) — the
  /// sim always allowed concurrent jobs; rivals always get an empty list.
  builds: BuildState[];
  /// BLOCKADE state (§contestable-territory), fog-safe: present only for the two
  /// participants — the besieger (`by_me`) and the owner (light-delayed). Third
  /// parties get null. `by` = the blockading corp; `since` = onset sim-time;
  /// `siege_since` = when the (defense-suppressed) capture clock started (§Part 2),
  /// null if the siege can't progress yet. Progress = (now−siege_since)/siege_secs.
  blockade: { by: PlayerId; since: number; by_me: boolean; siege_since: number | null } | null;
  /// Extractor upgrades built here (owner-only; rivals see 0).
  extractor_tier: number;
  /// Depot upgrades built here (§buildings step 2) — owner-only; rivals see 0.
  depot_tier: number;
  /// Shipyard upgrades built here (§buildings step 3) — owner-only; rivals see 0.
  /// Gates ship construction: Convoy needs ≥ 1, Raider ≥ 2.
  shipyard_tier: number;
  /// Sensor Array upgrades built here (§buildings step 2b) — owner-only; rivals
  /// see 0. Projects a standing sensor bubble for the owner.
  sensor_tier: number;
  /// Defense Platform tiers standing here (§buildings step 2c) — owner-only;
  /// rivals see 0 (a platform reveals itself only via engagement outcomes).
  defense_tier: number;
  /// Habitat tiers here (§buildings step 3a) — owner-only; rivals see 0.
  habitat_tier: number;
  /// Whether the Habitat's Provisions upkeep is covered — owner-only; rivals
  /// always see false. UNFED = boost suspended (nothing destroyed).
  habitat_fed: boolean;
  /// Fuel Refinery tiers here (§buildings step 3b) — owner-only; rivals see 0.
  refinery_tier: number;
  /// Development slots used/total (§buildings step 1) — owner-only; rivals see 0/0.
  slots_used: number;
  slots_total: number;
  /// Storage capacity + current fill, whole units (§buildings step 2) — owner-only;
  /// rivals see 0/0. `storage_used` may exceed the cap (grandfathered stockpile).
  storage_cap: number;
  storage_used: number;
  /// OUR scout-intel snapshot of this (rival) system, if any (§scout part 2) —
  /// delivered only once its light reached our command center. A SNAPSHOT: it
  /// ages and never auto-updates; re-scout to refresh.
  intel: IntelView | null;
  /// §syndicates Part 1: the (light-gated known) owner is a SYNDICATE ally as WE
  /// know it — drives the friendly ally tint. No owner-only data comes with it.
  ally?: boolean;
  /// §syndicates Part 3: OWNER-ONLY — an ally garrison hosted at THIS system (the
  /// coalition shield you're feeding): total ally garrison ships + whether their
  /// Provisions upkeep is covered. `0` = none; rivals always see 0.
  ally_garrison_ships?: number;
  ally_garrison_fed?: boolean;
  /// §node: this system's EXOTIC NODE, if any. bonus + awakened are PUBLIC (an
  /// awakened node is a galaxy-wide landmark); fed + region_radius are OWNER-ONLY.
  node?: NodeStateView;
  /// §explore R2: the EXACT deposit table — present iff WE have surveyed this
  /// system or own it (survey knowledge is permanent). Absent = unsurveyed:
  /// only the public band is known.
  deposits?: Deposit[] | null;
  /// §explore R3: the hidden TRAIT slug — CURRENT-OWNER-ONLY (never on a rival's
  /// wire; a survey doesn't reveal it). "bonus_vein:<commodity>" carries the
  /// vein's commodity; else "deep_deposits" | "unstable_geology" |
  /// "volatile_pockets" | "precursor_cache".
  trait?: string | null;
}

/// §node: the per-system view of an EXOTIC NODE — the midgame catalyst.
export interface NodeStateView {
  /// Stable bonus slug: "relay_anchor" | "veil" | "deep_scan".
  bonus: string;
  /// Human bonus title, e.g. "Relay Anchor".
  title: string;
  /// Has the node awakened (past the awakening time)? A dormant node is a
  /// telegraphed landmark; an awakened one is a live, capturable prize.
  awakened: boolean;
  /// OWNER-ONLY: is the node's upkeep met? An unfed node's bonus is SUSPENDED.
  fed: boolean;
  /// OWNER-ONLY: the node's region radius (sim units) for the holder's map ring.
  region_radius: number;
}

/// A stored scout-intel snapshot of a rival system's fortifications.
export interface IntelView {
  defense_tier: number;
  shipyard_tier: number;
  /// Sim-time of the ORIGINAL observation ("as of T₁") — the client ages it,
  /// even when the intel was relayed by an ally (§syndicates Part 2).
  observed_at: number;
  /// §syndicates Part 2 relay provenance — present only for ALLY-sourced intel
  /// (null for your own direct scout). `relayed_by` = the reporting ally's id
  /// (resolve to a name via the syndicate roster); `relayed_at` = T₂ (reached
  /// the ally's command center); `received_at` = T₃ (reached yours).
  relayed_by?: PlayerId | null;
  relayed_at?: number | null;
  received_at?: number | null;
  /// §pirates: the scouted PIRATE ENCLAVE tier at this system (0 = not an enclave).
  /// When > 0 the site is a pirate base; `defense_tier` is its base defense.
  enclave_tier?: number;
}

// A buildable thing + its recipe (§step1), sent once in the galaxy.
export interface BuildOption {
  key: string;
  label: string;
  costs: StockSlot[];
  build_secs: number;
}

export interface GalaxyInfo {
  hub: Vec2;
  radius: number;
  c: number; // speed of light, sim units / s
  sensor_range: number; // detection radius each of your assets projects
  raider_speed: number; // raider cruise speed — for the crude intercept estimate
  /// The sensor-bubble multiplier a SCOUT projects over the standard ship
  /// bubble (§scout) — for the coverage rendering.
  scout_sensor_mult: number;
  /// Sensor-array bubble tunables (§buildings step 2b): a tier-N array projects
  /// base + per_tier·(N−1) — for drawing our own arrays' coverage.
  sensor_array_base: number;
  sensor_array_per_tier: number;
  /// Defense Platform protection radius (§buildings step 2c) — for the subtle
  /// ring on our OWN defended systems (owner-only by construction).
  defense_platform_radius: number;
  /// Habitat tunables (§buildings step 3a): output ×mult^tier when fed; upkeep
  /// per_tier·tier Provisions/s — for the owner-only readout.
  habitat_output_mult: number;
  habitat_upkeep_per_tier: number;
  /// Refinery tunables (§buildings step 3b): rate·tier Volatiles/s in, yield
  /// Fuel out per Volatile — for the owner-only refining readout.
  refinery_rate_per_tier: number;
  refinery_yield: number;
  /// §contestable-territory Part 2: siege duration (sim s) — the client renders
  /// siege progress = (now − blockade.siege_since) / siege_secs.
  siege_secs: number;
  /// §pirates: the neutral PIRATE faction id — the client labels a fleet/report
  /// as pirate when its owner === this id (no name lookup needed).
  pirate_id?: PlayerId;
  /// §node: sim-time at which every EXOTIC system AWAKENS into a capturable node —
  /// the client telegraphs the countdown from t=0.
  node_awakening_time?: number;
  /// §node: a node's region radius (sim units) — for the holder's region ring.
  node_region_radius?: number;
  systems: SystemInfo[];
  build_options: BuildOption[]; // §step1 — what can be built + recipe costs/time
}

// §economy: the 12-commodity industrial web (5 raw · 5 processed · 2 advanced).
export type Commodity =
  | "metallic_ore" | "rare_elements" | "silicates" | "volatiles" | "biomass"      // raw
  | "alloys" | "electronics" | "polymers" | "fuel" | "provisions"                 // processed
  | "machinery" | "armaments";                                                    // advanced

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
  fuel_total: number; // §step1 — total Fuel across owned systems (fleet reserve)
}

// Economy news (mirrors sim TradeEvent, tagged by `event`).
export type TradeEvent =
  | { event: "Bought"; player: PlayerId; commodity: Commodity; units: number; unit_price: number }
  | { event: "Delivered"; player: PlayerId; commodity: Commodity; units: number }
  | { event: "SellDispatched"; player: PlayerId; commodity: Commodity; units: number }
  | { event: "Sold"; player: PlayerId; commodity: Commodity; units: number; unit_price: number }
  | { event: "LimitPlaced"; player: PlayerId; side: Side; commodity: Commodity; units: number; limit_price: number }
  | { event: "LimitFilled"; player: PlayerId; side: Side; commodity: Commodity; units: number; unit_price: number }
  | { event: "AutoDispatched"; player: PlayerId; commodity: Commodity; units: number; source: EntityId; rule_id: number }
  | { event: "SupplyDiverted"; player: PlayerId; commodity: Commodity; units: number; system: EntityId; action: DivertAction }
  | { event: "StorageOverflow"; player: PlayerId; commodity: Commodity; units: number; system: EntityId };

export type DivertAction = "lost" | "returned_home" | "sold_at_hub";

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

// --- Fleet doctrine (§16) — constrained combat & logistics policy. Mirrors the
// sim's serde enums exactly; the client reads it from the View and writes it via
// SetFleetDoctrine. Every field defaults to today's behaviour. ---
export type EngagementPolicy = "avoid" | "defensive_only" | "engage_weaker" | "engage_any";
// §offensive-orders Part 2: the per-fleet engagement POSTURE (mirrors the sim enum).
export type EngagementPosture = "passive" | "defensive" | "weapons_free";
export type RetreatThreshold = "quarter" | "half" | "three_quarter" | "never";
export type EscortPolicy = "guard_nearest" | "guard_richest" | "hold_station";
export type DestinationInvalidPolicy = "drop" | "return_home" | "sell_at_hub";
export interface FleetDoctrine {
  engagement: EngagementPolicy;
  retreat: RetreatThreshold;
  escort: EscortPolicy;
  destination_invalid: DestinationInvalidPolicy;
}
export function defaultDoctrine(): FleetDoctrine {
  return {
    engagement: "defensive_only",
    retreat: "never",
    escort: "guard_nearest",
    destination_invalid: "drop",
  };
}

export interface AnchorView {
  pos: Vec2;
  owner: PlayerId | null;
}

// --- Check-in timeline (§16, Layer 3) — the retained, server-composed digest of
// what became OBSERVABLE to the player, buffered across disconnects. ---
export type TimelineSeverity = "good" | "bad" | "warn" | "info";
export interface TimelineEntry {
  at_time: number; // sim-time the news became observable to this player
  severity: TimelineSeverity;
  text: string;
}

// A ship as the player perceives it — a delayed "ghost" (§6). `pos` is where
// the object was when its arriving light left it; `age` is how stale that is;
// `uncertainty` is how far it could have moved since (`age × max_speed`). This
// holds for OWN ships too — there is no FTL tether to your fleet, so a distant
// own ship is as uncertain as a distant enemy; `own` is only a "this is mine"
// marker, never a certainty grant.
// The estimated-size BUCKET for a fleet seen through the fog (§13.1 intel
// ladder). Deterministic classes `1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+` — an
// honest, un-invertible size estimate you get even for a far, out-of-coverage
// fleet. Mirrors the Rust `CountClass` serde form.
export type CountClass =
  | "one"
  | "two_to_three"
  | "four_to_seven"
  | "eight_to_fifteen"
  | "sixteen_to_thirty"
  | "thirty_one_plus";

// The human-facing bucket label ("est. 4–7 ships").
export function countClassLabel(c: CountClass): string {
  switch (c) {
    case "one": return "1";
    case "two_to_three": return "2–3";
    case "four_to_seven": return "4–7";
    case "eight_to_fifteen": return "8–15";
    case "sixteen_to_thirty": return "16–30";
    case "thirty_one_plus": return "31+";
  }
}

// One (kind, count) entry of a fleet's exact composition — present only in
// coverage / for your own fleets.
export interface CompCount {
  kind: ShipKind;
  count: number;
}

// A FLEET as you perceive it (§13.1). `kind` is the flagship (what it's drawn
// as). The two-tier intel ladder: `count_class` (size bucket) is ALWAYS present;
// `composition` (exact kinds + counts) only for your own fleets or a rival fleet
// inside your sensor coverage — never leaking the true count outside it.
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
  // Estimated-size bucket — always present on a visible fleet.
  count_class: CountClass;
  // Exact composition — present only in coverage or for your own fleet.
  composition: CompCount[] | null;
  // §Part 4 detection signature (how LOUD a dark fleet is; 1.0 = a lone raider at
  // full speed). Present only for dark fleets — drives the flare treatment.
  signature: number | null;
  // §offensive-orders Part 2 engagement posture — OWNER-ONLY (present for your own
  // fleets, null for every rival; a private standing policy that never leaks).
  posture: EngagementPosture | null;
  // §explore Part 2: SURVEY DWELL progress (0..1) — OWNER-ONLY (your fleet's own
  // order state); null/absent when not dwelling. Drives the progress ring.
  survey_progress?: number | null;
  // §syndicates Part 1: this fleet's owner is a SYNDICATE ally as WE know it
  // (light-delayed membership) — drives the friendly ally tint/pip.
  ally?: boolean;
  // §syndicates Part 3: OWNER-ONLY. When this is YOUR fleet stationed as an ally
  // garrison, the host system id it defends (else null); `garrison_fed` = the host
  // is covering its Provisions upkeep (else its defense is suspended).
  garrison_host?: EntityId | null;
  garrison_fed?: boolean;
  // §pirates: this fleet belongs to the neutral PIRATE faction (a raider pack) —
  // drives the distinct hostile-neutral tint. Hostile to everyone.
  pirate?: boolean;
}

// A fleet's transit throttle (§Part 4). `full` = formation speed (loud at flank);
// `stealth` = creep at STEALTH_FRACTION (quiet, ~2× trip).
export type TransitMode = "full" | "stealth";

// Total ship count implied by a ghost: exact when composition is known,
// otherwise null (you only have the bucket).
export function fleetExactCount(g: GhostView): number | null {
  if (!g.composition) return null;
  return g.composition.reduce((a, c) => a + c.count, 0);
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
  | { type: "ShipProduction"; system_id: EntityId }
  | { type: "SetStandingOrder"; order: StandingOrder }
  | { type: "ClearStandingOrder"; order_id: number }
  | { type: "SetFleetDoctrine"; doctrine: FleetDoctrine }
  // `join` (optional): a fleet docked at that system for the finished ship to
  // JOIN; omit / null forms a new fleet-of-one (§FLEETS management v1).
  | { type: "BuildShip"; system_id: EntityId; ship_kind: ShipKind; join?: EntityId | null }
  // §economy: the 16 structure slugs (the server accepts legacy slugs via alias).
  | { type: "DevelopSystem"; system_id: EntityId; upgrade: string }
  // §battles-take-time — withdraw an engaged fleet (light-delayed).
  | { type: "Withdraw"; fleet_id: EntityId }
  // §Part 4 — set a fleet's transit throttle (Full/Stealth).
  | { type: "SetFleetTransit"; fleet_id: EntityId; mode: TransitMode }
  // §FLEETS Part 3 — request a projected engagement estimate (read-only query).
  | { type: "EstimateEngagement"; attacker: EntityId; target: EntityId }
  // §FLEETS management v1 — compose fleets at an owned system.
  | { type: "MergeFleets"; into: EntityId; from: EntityId }
  | { type: "SplitFleet"; fleet_id: EntityId; counts: Record<ShipKind, number> | Partial<Record<ShipKind, number>> }
  // §contestable-territory Part 1 — order a raider fleet to blockade a rival system.
  | { type: "BlockadeSystem"; fleet_id: EntityId; system_id: EntityId }
  // §explore Part 2 — order a scout-carrying fleet to SURVEY a system's geology.
  | { type: "SurveySystem"; fleet_id: EntityId; system_id: EntityId }
  // §offensive-orders — attack a rival fleet (destroy); set a fleet's posture.
  | { type: "AttackFleet"; fleet_id: EntityId; target_id: EntityId }
  | { type: "SetFleetPosture"; fleet_id: EntityId; posture: EngagementPosture }
  // §syndicates Part 1 — alliance admin (instant owner-only).
  | { type: "CreateSyndicate"; name: string }
  | { type: "InviteToSyndicate"; name: string }
  | { type: "AcceptSyndicateInvite"; syndicate_id: SyndicateId }
  | { type: "LeaveSyndicate" }
  | { type: "DissolveSyndicate" }
  | { type: "Ping" };

// §syndicates Part 1: an alliance id (opaque decimal string on the wire).
export type SyndicateId = string;

// The viewer's OWN syndicate roster (never a rival's private roster).
export interface SyndicateView {
  id: SyndicateId;
  name: string;
  founder: PlayerId;
  is_founder: boolean;
  members: { id: PlayerId; name: string }[];
  invited: string[];
}

// A pending invitation the viewer may accept.
export interface SyndicateInviteView {
  id: SyndicateId;
  name: string;
}

/// §rankings: one corporation's row in the PUBLISHED leaderboard — a public
/// snapshot taken on the ledger close (the same table for every player). All values
/// are cumulative campaign totals. `player_id` matches your own id for the "your
/// row" highlight; `titles` are the category-leader chips this corp currently wears.
export interface RankingRow {
  player_id: PlayerId;
  name: string;
  valuation: number;
  trade_throughput: number;
  market_profit: number;
  cargo_captured: number;
  cargo_protected: number;
  battle_efficiency: number;
  battle_engagements: number;
  /// Whether `battle_efficiency` met the min-engagements floor (else provisional —
  /// shown but not ranked / title-eligible).
  battle_ranked: boolean;
  systems_developed: number;
  intel_gathered: number;
  recovery: number;
  titles: string[];
}

export type RaidOutcome =
  | "target_destroyed"
  | "attacker_destroyed"
  | "both_destroyed"
  | "both_survive"
  | "escaped";

// A delayed battle report (§8) — arrives on the recipient's own clock. Both
// sides observe the same result, light-delayed. §FLEETS Part 2: a
// composition-vs-composition report — `*_kind` are the flagships and `*_losses`
// list the per-kind ships each side lost over the Lanchester engagement.
export interface RaidReport {
  /// §battle-aftermath: stable id shared with the RETAINED copy in
  /// `View.battle_reports` — a news toast can open the same results panel.
  report_id: number;
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
  attacker_losses: CompCount[];
  target_losses: CompCount[];
}

// A projected engagement estimate (§FLEETS Part 3), computed server-side by
// running the SAME Lanchester attrition forward on YOUR view data. Honest about
// staleness: `target_known = false` means the target was out of sensor coverage,
// so it assumed a typical warfleet of the bucket size (never the true count).
export interface EngagementEstimate {
  attacker: EntityId;
  target: EntityId;
  own_losses: CompCount[];
  target_losses: CompCount[];
  own_survivors: CompCount[];
  target_survivors: CompCount[];
  target_known: boolean;
  target_count_class: CountClass;
  composition_age: number; // age of the target sighting (s)
  defenses_age: number | null; // age of the scouted-defenses snapshot (s), if any
  platform_tiers: number | null;
}

// §order-lifecycle: the flavor of a light-delayed order (mirrors sim OrderKind).
export type OrderKind = "move" | "raid" | "recall" | "withdraw" | "blockade" | "attack" | "survey";

// §battles-take-time: an ongoing battle as this player perceives it, light-gated.
// ONE battle entity = ONE map icon at `pos`; `participants` are the fleet ids
// revealed at the site (own + site-revealed rivals), used to SUPPRESS their
// individual markers (the icon carries the state) and to build the battle panel.
export interface BattleView {
  id: EntityId; // stable engagement id — keys the icon + selection
  pos: Vec2;
  age: number; // light delay of the sighting (s) — "battle raging, as of N ago"
  started_at: number; // sim-time the battle began (for observed-elapsed)
  own: boolean; // the viewer is one of the two sides
  participants: EntityId[]; // fleet ids in the fight (already revealed as ghosts)
}

// §battle-aftermath: a RETAINED concluded battle this player PARTICIPATED in —
// present only once their conclusion light arrived (`learned_at`). Powers the
// aftermath map marker + battle-results panel; survives reconnects (server
// keeps the last BATTLE_REPORTS_KEPT per player). Strictly owner-only.
export interface BattleReportView {
  id: number;
  pos: Vec2;
  at_time: number; // sim-time the battle concluded
  learned_at: number; // sim-time YOUR light arrived (when you learned)
  you: "attacker" | "defender";
  attacker_kind: ShipKind;
  target_kind: ShipKind;
  outcome: RaidOutcome;
  attacker_losses: CompCount[];
  target_losses: CompCount[];
}

// §contestable-territory Part 2: a retained CAPTURE this player participated in
// (per-participant, light-delayed) — powers the capture aftermath marker + panel.
// `captor` = you took the system; else you lost it. `plunder` = seized stockpile.
export interface CaptureReportView {
  id: number;
  pos: Vec2;
  at_time: number; // sim-time the system flipped
  learned_at: number; // sim-time YOUR light arrived
  captor: boolean;
  plunder: StockSlot[];
}

// One of the player's in-flight order lifecycles (OWNER-ONLY). The client derives
// the phase from `sim_time`: IN TRANSIT until `delivered_at`, AWAITING ECHO until
// `echo_at`, then confirmed (the entry drops). Both stamps are exact.
export interface PendingOrderView {
  fleet_id: EntityId;
  delivered_at: number;
  echo_at: number;
  kind: OrderKind;
}

// Server → client.
export type ServerMsg =
  | {
      type: "Welcome";
      player_id: PlayerId;
      name: string;
      // Wire protocol version (§FLEETS bumped it to 2) — a stale client can warn.
      protocol_version: number;
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
      doctrine: FleetDoctrine;
      // §order-lifecycle — the player's own in-flight order timestamps (owner-only).
      pending_orders: PendingOrderView[];
      // §battles-take-time — ongoing battles visible to this player (light-gated).
      battles: BattleView[];
      /// §battle-aftermath: retained concluded-battle reports (owner-only).
      battle_reports: BattleReportView[];
      /// §contestable-territory Part 2: retained capture reports (per-participant).
      capture_reports: CaptureReportView[];
      /// §syndicates Part 1: the viewer's OWN syndicate roster (null if none).
      syndicate?: SyndicateView | null;
      /// §syndicates Part 1: pending invitations the viewer may accept.
      syndicate_invites?: SyndicateInviteView[];
      /// §rankings: the PUBLISHED leaderboard — public, same for every player,
      /// snapshotted on the ledger close (holds steady between closes).
      rankings?: RankingRow[];
    }
  | { type: "Report"; report: RaidReport }
  | { type: "Timeline"; entries: TimelineEntry[]; away_since: number }
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
  | ({ type: "EngagementEstimate" } & EngagementEstimate)
  | { type: "Error"; message: string };

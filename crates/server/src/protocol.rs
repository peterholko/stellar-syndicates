//! The WebSocket wire protocol between client and server.
//!
//! Messages are versioned MessagePack binary frames, tagged by a `type` field.
//! The client sends *intents*
//! ([`ClientMsg`]); the server pushes each player their own *filtered view*
//! ([`ServerMsg`]). The client holds no authoritative state — these messages
//! are the entire contract (§14).
//!
//! Views contain each player's delayed/fogged reconstruction, never server
//! truth. The transport serializes these explicit DTOs only; changing the
//! encoding must never move or bypass a visibility gate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sim::{
    Commodity, CountClass, EngagementPolicy, EngagementPosture, EntityId, FleetDoctrine, OrderKind,
    PlayerId, RaidOutcome, RankingRow, ShipKind, Side, StandingOrder, StructureKind, SyndicateId,
    TradeEvent, TransitMode, Vec2,
};

/// v78: §gun-platform — the `build_gun_platform` founding stage (step 6; the
/// charter has 22 steps), the six-level station ladder from the Gun Platform,
/// `VendettaView.fallen`, `StrikeView.target` and `.wrecked`, the
/// `freighter` strike, and `ClientMsg::RestartCorporation`.
/// v77: §vendetta-campaign — `VendettaView.deadline_at` and `.strikes`, the
/// `mine_strike` and `barge_strike` beats.
/// v76: §refining + §den-pocket — the `build_smelter` and `prospect_belt`
/// founding stages (steps 14 and 18; the charter has 21 steps).
/// v75: §season — `Sections.season`: galaxy maturity and the Authority's grip
/// from the census this corporation has heard, and the Galactic News Network
/// items whose light from the Hub has reached it; the `decree` log line.
/// v74: §scouting — `SystemStateView.scouted_stores` (a scout's look at a rival's
/// stores in raiders' reach and its Vault) and counter-scouting log entries.
/// v73: §vault + §raid-order — the Vault structure (`vault`), `BlockadeSystem.raid`,
/// an owner-only fleet `raiding` flag and the `needs_hold` refusal.
/// v72: §unlocks — `ResearchView.structure_tiers`: the highest level of each
/// structure this corporation may build now, so the client renders nothing
/// research hasn't unlocked (station levels, tiers V–VI, gated buildings).
/// v71: §log — `TimelineEntry` gains `id`, `kind`, `group`, `phase`, `refs`
/// and `data`; the journal keeps 300 entries or 7 days; operation churn,
/// order echoes and pirate-raid lines are merged or dropped at the source.
/// v70: §traders — `GhostView.trader`: which free trader flies a merchant
/// ship, so each draws its own ship (Long Odds, Clean Slate, Second Wind,
/// Small Fortune).
/// v69: §holds — the `combat_freighter` hull: a raiding freighter whose
/// hold takes only plunder, seized cargo and finds.
/// v68: §treasury — `WalletView.credits` is the treasury at the command
/// center, known at once (it was the Hub's light-delayed report); an order to
/// the Hub takes its payment as it leaves. Same shape, new meaning: clients
/// stop holding credits for orders in flight.
/// v67: §traders — `FoundingView.trader`: which of four free traders
/// (`teo`, `ines`, `mags`, `rafi`) flies this founder's merchant.
/// §privateer-hunt — `FoundingView.privateer_last_seen`: the privateer's
/// hiding spot once the raid is over (it is tracked only during the raid).
/// v66: §charter-gates — `FoundingView.charter_gates`: the structures the
/// charter has yet to open, by step.
/// v65: Shipyard I builds the Interceptor (`raider`); the guide skips
/// Shipyard II.
/// v64: §asteroid-quarry — the `asteroid_quarry` structure (a belt's one
/// structure) and the `no_supplies` line state.
/// v63: §vendetta — the `vendetta` founding stage (step 19),
/// `FoundingView.pirates` and `FoundingView.vendetta`.
/// v62: §node — `SystemInfo.exotic`: the server decides which stars are
/// exotic (a home never is), and the client's star art follows it.
/// v61: §social (R5) the syndicate channel, mail, mute/report and
/// `Sections.social`; §welcome (R9) `Timeline.welcome`; §settle-near-friends
/// (R7) CreateInvite/RevokeInvite/RedeemInvite, `InviteStatus` and
/// `SocialView.invites`; §syndicate-projects (R6) `SyndicateView.projects`,
/// ContributeSyndicateProject, RaiseSyndicateBonus and
/// `Sections.syndicate_rankings`.
/// v60: §research-wiring — the eight once-hidden programmes: the
/// `lance_array` module and `GhostView.projected` (Predictive Plots).
/// v59: §research-wiring (R2) — `ClientMsg::SetDesignation` and
/// `ResearchView.designations`; research effects served where they show.
/// v58: §leave + §tier-curve (R1/R4) — `ClientMsg::SetLeave` and the owner's
/// `View.leave`; structure tiers past II follow the tier curve (client mirror).
/// v57: §founding — the home world's type is dealt, not chosen:
/// `FoundingOffer.profile` names it and `FoundColony` carries no choice.
/// v56: §real-coordinates — `BodyView.orbit`: every charted world's orbit
/// (radius, angle at time 0, period), so worlds and the ships at them are
/// drawn where the sim has them; belts carry their laid-out ring.
/// v55: §signals — `View.signals`, AnswerSignal, DismissSignal, the
/// `signal` expedition task and `CaptainRosterView.hurt_until`; the first
/// finds' news lines retired into the signals' first ten.
/// v54: §medals (`RankingRow.medals`), §contract-board (`View.board`,
/// `OperationView.board_day`) and §first-finds (news lines only).
/// v53: §charter-v5 — the pirate raid: the `pirate_raid` and `sell_ore`
/// stages, `FoundingView.raid_seen` / `raided` (for `counterfeit_revealed` /
/// `ambushed`).
/// v52: §charter-v4 — the privateer den: the `eliminate_den` stage and
/// `FoundingView.den` / `den_cleared`.
/// v51: §charter-v4 — the bare landing and the counterfeit semiconductors:
/// eight new `FoundingStage`s (build_bioharvester … expand_mining) and
/// `FoundingView.counterfeit_revealed` / `ambushed` / `land_grant_open`.
/// v50: §merchant — the traveling merchant: MerchantTrade, the owner-only
/// `merchant` catalogue and the public `GhostView.merchant` flag.
/// v49: §fuel-overhaul — Uranium/Deuterium/Antimatter fleet stores
/// (`GhostView.uranium`, `helium3`, `antimatter` and capacities), the Slow
/// transit mode, solar plants (`PowerView.fuel` optional), SetDepotRule and
/// the corp's served depot rules, new goods and structures.
/// v48: §power — per-planet grids (`BodyView.power`), the Power Plant, and
/// Power in the factor chains (`AssignmentView.power`, `AcademyRow.power`).
/// v47: §rare-materials — each research catalog entry carries its goods
/// basket (`supplies`).
/// v46: §belts phase 4 — the Mining Belt Station: AnchorStation,
/// UnanchorStation, StationLoad, station freight stops and a fleet's anchor.
/// v45: §pockets — ProspectBelt, MineBelt `pocket`, belt pocket rows and the
/// six pocket commodities.
/// v34: advanced industry goods and static factory recipes/research prerequisites.
/// v89: §privateer-contract — the Authority's contract on a founder's
/// privateer (`OperationView.briefing.follow_up` "founder_privateer"),
/// offered as the hunt begins, with its "Authority contract offered · …" line.
/// v88: §watch-the-pirate — `FoundingView.raid_lost`: the colony has lost
/// the privateer after its raid (the guide's "Watch the pirate" ends).
/// v87: §charter-advisor — the Authority officer dealt to guide this
/// charter (`FoundingView.advisor`, 0–2), docs/charter-advisor.md.
/// v86: §licences — the Authority's hull licences: `CharterView.licences`
/// (and `FoundingView.raid_over`: the raider over the colony, as heard).
/// (each Orbital Shipyard and Capital Slipway hull: held, its price, or why
/// it isn't for sale), `ClientMsg::BuyLicence`, the `needs_licence` build
/// refusal and the licence log lines (docs/blueprint-licences.md).
/// v85: §modules-as-goods — the seventeen modules are goods (`Commodity`):
/// no module ledger (`SystemStateView.modules`), no module crates aboard
/// (`GhostView.modules`), no `TransferModules` / `BuyModule` / `SellModule`.
/// v84: no charter crate after the raid (`CharterCrateId` has no `raid`;
/// the landing kit carries its goods), and two charter steps: `full_speed`
/// (the freighter's Transit before the loot) and `answer_signal` (while
/// Enrichment runs).
/// v83: §stash — the den's aftermath, three steps (`repair_interceptor`,
/// `build_tiny_freighter`, `haul_loot`): the den's loot
/// (`FoundingView.stash`) and the hulls its blast hurt
/// (`FoundingView.den_wounded`).
/// v82: §migration-status — each own world's housing and liner status
/// (`BodyView.migration`).
/// v81: §habitat-rebase — the camp's Permanent Quarters project is gone
/// (`SettlementProject` has no `quarters`); a Habitat level houses 8,000
/// (`pop_cap_per_habitat_tier`).
/// v80: §player-styles — the daily loot cap (`View.raid_losses`), the rival
/// raid warning (`RaidSighted` lines), repeating raids (`BlockadeSystem.repeat`,
/// `GhostView.raid_trip`) and personal pirate hideouts.
/// v79: §charter-crates — the charter's supplies as crates released a
/// chapter at a time (`FoundingView.crates`, `ClientMsg::CollectCharterCrate`)
/// and the "Secure your stores" step (`secure_stores`).
/// Binary MessagePack transport: bump the WebSocket subprotocol and the
/// client's wire version together; old text clients must refresh, not silently
/// misinterpret binary data. Welcome retains the version for the UI contract.
/// v90: §claims — claims bought from the Authority (`ClientMsg::BuyClaim`,
/// `CharterView.claims`, `SystemInfo.reserved`).
/// v91: §sell-alloys — the trader buys Alloys: `MerchantView.buys`,
/// `buy_price` and `buy_room` (were `ore_price` and `ore_room`).
pub const PROTOCOL_VERSION: u32 = 91;

/// Messages sent by the client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    /// Land on the home world the server dealt (`ServerMsg::FoundingOffer`).
    FoundColony,
    /// §gun-platform: found the corporation again on its home once its colony
    /// fell to the uncle (`VendettaView.fallen`). Refused otherwise.
    RestartCorporation,
    /// §charter-crates: collect a released charter crate into the home
    /// stockpile (`FoundingView.crates`). Refused before its step.
    CollectCharterCrate { charter_crate: sim::CharterCrate },
    DevelopSettlement { system_id: EntityId, body_id: u32, project: sim::settlement::SettlementProject },
    SetRefiningReserve { system_id: EntityId, ore: u32, fuel: u32 },
    /// §route-edit: `revise` keeps a run under way in its place.
    SetFreightRoute { fleet_id: EntityId, route: Option<sim::industry::FreightRoute>, #[serde(default)] revise: bool },
    DeployOutpost { fleet_id: EntityId, system_id: EntityId, body_id: u32,
        outpost: sim::industry::OutpostKind, commodity: Commodity },
    SkimFuel { fleet_id: EntityId, system_id: EntityId },
    /// §belts: send a fleet's Mining Barges to work one ore of a charted belt.
    MineBelt { fleet_id: EntityId, system_id: EntityId, body_id: u32, ore: Commodity,
        /// §pockets: work the found pocket of `ore`, not the belt's own ore.
        #[serde(default)]
        pocket: bool },
    /// §pockets: prospect a charted belt with a Scout or Mining Barge.
    ProspectBelt { fleet_id: EntityId, system_id: EntityId, body_id: u32 },
    /// §belts: anchor a lone Mining Belt Station at a charted belt.
    AnchorStation { fleet_id: EntityId, system_id: EntityId, body_id: u32 },
    /// §belts: free an anchored station, or stop one on its way.
    UnanchorStation { fleet_id: EntityId },
    /// §belts: load an idle freighter beside your station from its hold.
    StationLoad { fleet_id: EntityId, station_id: EntityId, commodity: Commodity, units: u32 },
    ReserveProject { system_id: EntityId, target: sim::industry::ProjectTarget, reserve: bool },
    StartColonyProject { system_id: EntityId, body_id: u32,
        project: sim::industry::ColonyProjectKind, commodity: Commodity },
    SetColonyProjectActive { system_id: EntityId, project: sim::industry::ColonyProjectKind, active: bool },
    /// First application message: ready to receive the game. Identity comes
    /// exclusively from the authenticated HTTP handshake; `name` is retained
    /// for wire compatibility but cannot select or rename a corporation.
    /// Missing `view_hz` retains the desktop 10 Hz cadence.
    Join {
        name: String,
        #[serde(default)]
        view_hz: Option<u8>,
    },

    /// Order one of the player's own ships to a destination. Travels at light
    /// speed to the ship (§6); the server attaches the issuing player.
    MoveShip {
        ship_id: EntityId,
        dest: Vec2,
    },

    /// Cancel the fleet's current course and hold at its true position when
    /// this ordinary light-delayed command arrives.
    HoldFleet { ship_id: EntityId },

    /// Order a jump-capable fleet to spool and relocate. The sim validates the
    /// true fleet, fuel, range, and both gravity-well endpoints when this
    /// light-delayed command arrives.
    JumpShip {
        ship_id: EntityId,
        dest: Vec2,
    },

    /// §emplacements: the named Construction Ship builds a structure WHERE IT
    /// IS PARKED — the player flies it to the spot first, then orders the
    /// build. Siting and the kit charge are the sim's business; the server only
    /// attaches the issuing player. Field is `emplacement` (not `kind`) because
    /// the enum's own tag is already `type`.
    BuildEmplacement {
        builder: EntityId,
        emplacement: sim::emplace::EmplacementKind,
    },

    /// §emplacements: send the named COMBATANT fleet to tear down a rival's
    /// structure. The sim validates ownership, teeth, and the ally rule.
    DemolishEmplacement {
        fleet: EntityId,
        target: EntityId,
    },

    /// Commit one of the player's raiders to intercept a target ship (§8).
    CommitRaid {
        raider_id: EntityId,
        target_id: EntityId,
    },

    /// Assign a dedicated Interceptor to shadow and defend another owned fleet.
    GuardFleet {
        interceptor_id: EntityId,
        target_id: EntityId,
    },
    RefuelFleet {
        fleet_id: EntityId,
        target_id: EntityId,
    },
    /// Raise an interdiction field where the fleet is when the order arrives.
    DeployInterdiction {
        fleet_id: EntityId,
    },
    DefendSystem {
        fleet_id: EntityId,
        system_id: EntityId,
        pursuit_radius: f64,
    },

    /// Recall a raider (break off, return home). May arrive too late (§8).
    RecallRaid {
        raider_id: EntityId,
    },

    /// Buy at the Global Market (§9, §TCA): instant settlement into the
    /// corporation's Market Warehouse. Moving the lot onward is a separate
    /// instruction at the Warehouse freight desk.
    MarketBuy {
        commodity: Commodity,
        units: u32,
        #[serde(default)]
        max_unit_price: Option<f64>,
    },
    /// §merchant: one exchange with the merchant docked at the player's home.
    MerchantTrade {
        #[serde(default)]
        buys: BTreeMap<Commodity, u32>,
        #[serde(default)]
        sells: BTreeMap<Commodity, u32>,
    },

    /// Sell at the Global Market (§9, §TCA): draws from the corp's
    /// warehouse and settles instantly at the standing price.
    MarketSell {
        commodity: Commodity,
        units: u32,
        #[serde(default)]
        min_unit_price: Option<f64>,
    },

    /// §TCA Part 5: player-convoy logistics — load/unload across the Market Hub
    /// warehouse or an owned system's stockpile, and the haul order that sends a
    /// loaded hull to the Market Hub (optionally selling on arrival).
    HubLoad {
        fleet_id: EntityId,
        commodity: Commodity,
        units: u32,
    },
    HubUnload {
        fleet_id: EntityId,
    },
    SystemLoad {
        fleet_id: EntityId,
        system: EntityId,
        commodity: Commodity,
        units: u32,
    },
    SystemUnload {
        fleet_id: EntityId,
        system: EntityId,
    },
    HaulToMarketHub {
        fleet_id: EntityId,
        #[serde(default)]
        sell_on_arrival: bool,
    },
    /// Send a loaded player freighter from its Hub berth to an owned system;
    /// its mixed manifest unloads into that system on arrival.
    HaulToSystem {
        fleet_id: EntityId,
        system: EntityId,
    },
    /// Dispatch an Authority Astral Assistance tender to a fuel-stalled fleet.
    /// §fuel-overhaul: keep `target` units of a fuel at one of your systems,
    /// shipped by the Authority (0 clears the rule).
    SetDepotRule { system_id: EntityId, commodity: Commodity, target: u32 },
    RequestFuelRescue {
        fleet_id: EntityId,
    },

    /// §TCA Phase 2: buy charter standing back from the Authority (credits burned,
    /// clamped to the ceiling — you pay only for points actually restored).
    PayReinstatement {
        points: f64,
    },

    /// §licences (v86): buy the Authority's licence to build `hull` at the
    /// Hub (docs/blueprint-licences.md). It travels with its payment.
    BuyLicence {
        hull: ShipKind,
    },

    /// §claims (v90): buy the Authority's claim to colonize `system`. It
    /// travels to the Hub with its payment.
    BuyClaim {
        system: EntityId,
    },

    /// §season (v75): answer the Authority's levy rise (refused, the good is
    /// embargoed until the arrears are paid).
    AnswerLevy {
        accept: bool,
    },
    /// §season (v75): pay the arrears that lift an embargo.
    PayArrears {
        commodity: Commodity,
    },
    /// §season (v75): pay a requisition, or refuse it.
    AnswerRequisition {
        id: u64,
        pay: bool,
    },
    /// §season (v75): feed the Keystone on a sealed world from its stockpile.
    FeedKeystone {
        system_id: EntityId,
        commodity: Commodity,
        units: u32,
    },
    /// §season (v75): Gate Step a fleet to the Keystone world or a held relic.
    GateStep {
        fleet_id: EntityId,
        system_id: EntityId,
    },

    /// §TCA: toggle whether one of the player's BLOCKADING fleets also engages
    /// Authority freight arriving at the strangled system. Instant local policy.
    SetEngageFreight {
        fleet_id: EntityId,
        on: bool,
    },

    /// §blockade-running: set whether one of the player's freighter fleets runs
    /// a held blockade at its destination instead of waiting outside it.
    /// Light-delayed to the fleet like any configuration.
    SetRunBlockades {
        fleet_id: EntityId,
        on: bool,
    },

    /// §TCA: book OUTBOUND Authority freight — warehouse → an owned system.
    BookFreightOut {
        system: EntityId,
        commodity: Commodity,
        units: u32,
    },

    /// §TCA: book INBOUND Authority freight — an owned system → the warehouse,
    /// optionally sold at the Exchange the moment it lands.
    BookFreightIn {
        system: EntityId,
        commodity: Commodity,
        units: u32,
        #[serde(default)]
        sell_on_arrival: bool,
    },

    /// Place a resting limit order; it clears in the periodic batch (§9).
    PlaceLimitOrder {
        side: Side,
        commodity: Commodity,
        units: u32,
        limit_price: f64,
    },
    /// Cancel one of the issuing corporation's resting limit orders.
    CancelLimitOrder {
        order_id: u64,
    },

    /// Ship a claimed system's accumulated production to the hub to sell (§9) —
    /// spawns raidable convoys from the system.
    ShipProduction {
        system_id: EntityId,
    },

    /// Supply a system: move goods from the corp's HUB WAREHOUSE into an owned
    /// system's stockpile via a raidable convoy sailing from the hub — the bridge
    /// that lets market-bought inputs feed a system's converters, and the free
    /// (but interceptable) alternative to booking Authority freight.
    StockSystem {
        system_id: EntityId,
        commodity: Commodity,
        units: u32,
    },

    /// Create or replace a standing logistics order (§15). `order.id == 0` creates;
    /// a matching id edits. The instruction travels to its source system.
    SetStandingOrder {
        order: StandingOrder,
    },

    /// Remove a standing order by id.
    ClearStandingOrder {
        order_id: u32,
    },

    /// Remove one terminal LOST fleet-order row after its jump-loss news has
    /// arrived. The sim accepts only an owned order already marked lost.
    DismissLostOrder {
        order_id: u64,
    },

    /// Set the corporation's fleet doctrine (§16) — the constrained combat &
    /// logistics policy. Instant local administration; the server attaches the
    /// issuing player.
    SetFleetDoctrine {
        doctrine: FleetDoctrine,
    },

    /// Build a ship at one of the player's owned systems (§step1 growth sink) — costs
    /// a commodity recipe from that system's stockpile and completes over time.
    /// `join` (optional) names a fleet docked at that system for the finished ship
    /// to JOIN; omitted / `null` forms a new fleet-of-one (§FLEETS management v1).
    BuildShip {
        system_id: EntityId,
        ship_kind: ShipKind,
        #[serde(default)]
        join: Option<EntityId>,
        /// §modules Part B4: the loadout to fit the ship with at build — must be
        /// ≤ the hull's slots and covered by the system's module ledger (both
        /// debited). serde default = unfitted (old clients build stock hulls).
        #[serde(default)]
        loadout: sim::Loadout,
        /// §reactors-drives: the reactor to build with (the recipe's Fission
        /// Reactors become this rung's). serde default = Fission.
        #[serde(default)]
        reactor: sim::fuel::ReactorGrade,
        /// The warp drive to build with (the recipe's Zephyr Drives become this
        /// rung's); the reactor must power it. serde default = Zephyr.
        #[serde(default)]
        drive: sim::fuel::Drive,
    },

    /// §reactors-drives: refit `n` hulls of `ship` in a docked fleet to
    /// `reactor` and/or `drive` (None keeps that part) at an owned or allied
    /// Shipyard.
    RefitEngines {
        fleet_id: EntityId,
        ship: ShipKind,
        #[serde(default)]
        reactor: Option<sim::fuel::ReactorGrade>,
        #[serde(default)]
        drive: Option<sim::fuel::Drive>,
        n: u32,
    },

    /// §orbital-defense: refit one hardpoint of the system's station (needs an
    /// Ordnance Foundry there; crates come from the system's module ledger).
    FitStation {
        system_id: EntityId,
        mount: u8,
        #[serde(default)]
        fit: sim::Loadout,
    },

    /// Develop one of the player's owned systems (§step1 structure sink), e.g. an
    /// Extractor tier that raises its output — costs a recipe, completes over time.
    DevelopSystem {
        system_id: EntityId,
        upgrade: StructureKind,
        /// §bodies: the body to build on; omitted (old clients) auto-sites.
        #[serde(default)]
        body_id: Option<u32>,
    },

    /// §construction: cancel a structure still waiting for its world's work
    /// slot (`job_id` from that system's served build list). Light-delayed;
    /// the goods return to the system stockpile on arrival.
    CancelBuild {
        system_id: EntityId,
        job_id: u64,
    },

    /// §economy Part 3: post workforce crews (and §Part 4: specialists from the
    /// resident pool) to a structure at one of the player's owned systems (all
    /// zero clears the line). Instant local administration; clamps server-side.
    SetAssignment {
        system_id: EntityId,
        structure: StructureKind,
        workers: u32,
        #[serde(default)]
        specialists: BTreeMap<sim::SpecialistKind, u32>,
        /// §bodies: the body whose line this staffs; omitted targets the holder.
        #[serde(default)]
        body_id: Option<u32>,
        #[serde(default)]
        refining_ore: Option<Commodity>,
    },

    /// Owner-only civilian immigration policy for one inhabited body.
    SetMigrationPolicy {
        system_id: EntityId,
        body_id: u32,
        policy: sim::MigrationPolicy,
    },

    /// §veil: owner-only — raise this system's Veil under a held blockade, or
    /// keep it down (batteries firing, fuel kept).
    SetVeilPolicy { system_id: EntityId, raise: bool },

    /// Relocate one physical workforce cohort between owned colonies.
    RelocateMigrants {
        from_system: EntityId,
        from_body: u32,
        to_system: EntityId,
        to_body: u32,
    },

    /// §economy Part 4: sign a Sol specialist contract — credits now, a
    /// personnel convoy hub → dest (sub-light, raidable, manifest fogged).
    HireSpecialist {
        specialist: sim::SpecialistKind,
        dest_system: EntityId,
    },

    /// §economy Part 4: enqueue an Academy training course (needs Academy ≥ 1).
    TrainSpecialist {
        system_id: EntityId,
        specialist: sim::SpecialistKind,
    },

    /// §economy Part 4: carry resident specialists between owned/allied systems
    /// on a dedicated personnel convoy.
    TransferSpecialists {
        from: EntityId,
        to: EntityId,
        manifest: BTreeMap<sim::SpecialistKind, u32>,
    },

    /// §modules Part B3: manufacture one module into the system's ledger (needs an
    /// Armaments Complex ≥ 1; basic utilities use a staffed Shipyard I and their
    /// manufacturing research). Costs goods; rides the build queue.
    BuildModule {
        system_id: EntityId,
        module: sim::ModuleKind,
    },

    /// §modules Part B4: refit `n` ships of `ship`/`from` in a docked fleet to a
    /// new `to` loadout at a Shipyard the player owns or is allied with.
    RefitShips {
        fleet_id: EntityId,
        ship: ShipKind,
        from: sim::Loadout,
        to: sim::Loadout,
        n: u32,
    },

    /// WITHDRAW an engaged fleet from its battle (§battles-take-time) — a coarse,
    /// light-delayed break-off order.
    Withdraw {
        fleet_id: EntityId,
    },

    /// Set a fleet's TRANSIT throttle (§Part 4): Full or Stealth. Instant local
    /// administration on the player's own fleet.
    SetFleetTransit {
        fleet_id: EntityId,
        mode: TransitMode,
    },

    /// Ask for a PROJECTED engagement estimate (§FLEETS Part 3): if `attacker`
    /// (one of the player's fleets) raided `target`, what would the losses be?
    /// Computed from the player's OWN view data only — exact where they have
    /// sensor coverage, an honest typical-hull estimate where they don't.
    EstimateEngagement {
        attacker: EntityId,
        target: EntityId,
    },

    /// Merge one of the player's fleets INTO another (§FLEETS management v1). Both
    /// must be the player's, idle, and docked together at an owned system.
    MergeFleets {
        into: EntityId,
        from: EntityId,
    },

    /// Split ships off one of the player's fleets into a new fleet at an owned
    /// system (§FLEETS management v1). `counts` = how many of each kind to detach.
    SplitFleet {
        fleet_id: EntityId,
        counts: BTreeMap<ShipKind, u32>,
    },

    /// BLOCKADE a rival system (§contestable-territory Part 1): order one of the
    /// player's fleets (must contain a raider) to take station on a rival's
    /// system and strangle its logistics. Light-delayed like a move order.
    BlockadeSystem {
        fleet_id: EntityId,
        system_id: EntityId,
        /// §raid-order (v73): load until full, then bring the loot home.
        #[serde(default)]
        raid: bool,
        /// §player-styles (v80): a raid that sets out again after each trip.
        #[serde(default)]
        repeat: bool,
    },

    /// SURVEY a system's exact geology (§explore Part 2): order one of the
    /// player's fleets (must contain a Scout) to fly on-site and dwell. Valid on
    /// ANY system (pre-siege prospecting intended). Light-delayed like a move.
    SurveySystem {
        fleet_id: EntityId,
        system_id: EntityId,
    },
    ExploreSite {
        fleet_id: EntityId,
        site_id: EntityId,
        task: sim::sites::ExpeditionTask,
    },
    AnnotateExploration {
        entry: sim::sites::JournalEntry,
    },
    /// §signals: send a captained fleet to sweep one of your signals.
    AnswerSignal {
        fleet_id: EntityId,
        signal_id: EntityId,
    },
    /// §signals: clear an answered signal's report card.
    DismissSignal {
        signal_id: EntityId,
    },

    /// ATTACK a rival fleet (§offensive-orders Part 1) — the targeted destroy verb.
    /// Orderable on any rival fleet; the attacker must contain a raider. Light-
    /// delayed like a raid; on contact it's a FULL battle (destroy, cargo lost),
    /// unlike CommitRaid (steal).
    AttackFleet {
        fleet_id: EntityId,
        target_id: EntityId,
    },

    /// Set a fleet's ENGAGEMENT POSTURE (§offensive-orders Part 2): Passive /
    /// Defensive / WeaponsFree. Instant local administration on the player's own
    /// fleet (a standing per-fleet policy, like SetFleetTransit).
    SetFleetPosture {
        fleet_id: EntityId,
        posture: EngagementPosture,
    },
    SetFleetMission { fleet_id: EntityId, mission: sim::doctrine::MissionProfile },
    RecruitCaptain {
        system_id: EntityId,
    },
    AssignCaptain {
        captain_id: u32,
        fleet_id: EntityId,
    },
    ReserveCaptain {
        captain_id: u32,
    },
    TrainCaptain {
        captain_id: u32,
        attribute: sim::CaptainAttribute,
    },

    // ---- SYNDICATES (§syndicates Part 1) -------------------------------------
    /// FOUND a syndicate with the caller as founder. The server attaches the
    /// issuing player.
    CreateSyndicate {
        name: String,
    },
    /// INVITE a corp (BY NAME — resolved server-side to its stable id) into the
    /// caller's syndicate. Founder-only; ignored if the name isn't a joined corp.
    InviteToSyndicate {
        name: String,
    },
    /// ACCEPT a pending invitation to the named syndicate.
    AcceptSyndicateInvite {
        syndicate_id: SyndicateId,
    },
    /// LEAVE the caller's syndicate.
    LeaveSyndicate,
    /// DISSOLVE the caller's syndicate (founder-only).
    DissolveSyndicate,
    SetSyndicateRole {
        member: PlayerId,
        role: sim::SyndicateRole,
    },

    // ---- OPERATIONS ---------------------------------------------------------
    AcceptOperation {
        operation_id: sim::OperationId,
    },
    AbandonOperation {
        operation_id: sim::OperationId,
    },
    AssignOperationFleet {
        operation_id: sim::OperationId,
        fleet_id: EntityId,
        #[serde(default)]
        protected_fleet: Option<EntityId>,
    },
    RecoverOperation {
        operation_id: sim::OperationId,
        fleet_id: EntityId,
    },
    ContributeOperationCargo {
        operation_id: sim::OperationId,
        commodity: Commodity,
        units: u32,
    },
    CreateSyndicateOperation {
        system_id: EntityId,
    },
    /// §syndicate-projects (R6): give the rung's goods from one of your worlds.
    ContributeSyndicateProject {
        system_id: EntityId,
        commodity: Commodity,
        units: u32,
    },
    /// Spend project points on a bonus level.
    RaiseSyndicateBonus {
        bonus: sim::syndicate::SyndicateBonus,
    },

    // ---- DIPLOMACY ----------------------------------------------------------
    ProposeTreaty {
        target_name: String,
        treaty: sim::diplomacy::TreatyKind,
    },
    RespondTreaty {
        proposal_id: u64,
        accept: bool,
    },
    DeclareWar {
        target_name: String,
    },
    CancelTreaty {
        target: PlayerId,
    },

    // ---- RESEARCH (§research R6) --------------------------------------------
    /// SET the caller's corporation research QUEUE (ordered programme ids). The
    /// front promotes to the active programme; the sim validates + soft-rejects
    /// unknown/hidden/completed ids. CC-local, no positional delay.
    SetResearchQueue {
        queue: Vec<String>,
    },

    // ---- FITTING (§fitting Stage A) ------------------------------------------
    /// SAVE a doctrine fit (named hull + loadout) on the caller's syndicate.
    /// The sim validates slots + fitting budget; same-name replaces. CC-local.
    SaveFit {
        name: String,
        ship: ShipKind,
        #[serde(default)]
        loadout: sim::Loadout,
    },
    /// DELETE a doctrine fit by name from the caller's syndicate. CC-local.
    DeleteFit {
        name: String,
    },
    /// §ladder B4: NAME the syndicate's flagship Titan (empty un-christens).
    NameFlagship {
        name: String,
    },
    /// §leave (R4): go on leave (`on`) or come back.
    SetLeave {
        on: bool,
    },
    /// §research-grant (R3): buy one research-hour for the active programme.
    FundResearch,
    /// §designations (R2): point a researched capability at its target.
    SetDesignation {
        cap: sim::research::Cap,
        target: sim::research::DesignationTarget,
    },
    /// §social (R5): a line on the syndicate channel.
    SyndicateSay { text: String },
    /// §social: mail another corporation, by its name.
    SendMail { to: String, subject: String, body: String },
    MarkMailRead { id: u64 },
    DeleteMail { id: u64 },
    /// §social: hide (or show again) a corporation's lines and mail.
    MuteCorp { player: PlayerId, mute: bool },
    /// §social: report a channel line (`kind` "chat") or a received mail
    /// ("mail") to the operators.
    ReportMessage { kind: String, id: u64, reason: String },
    /// §settle-near-friends (R7): a link that holds the free home slot nearest
    /// the caller's home, for a friend.
    CreateInvite,
    RevokeInvite { code: String },
    /// A newcomer's link, before `FoundColony`: the landing takes the held
    /// slot and joins the inviter's syndicate. Answered with `InviteStatus`.
    RedeemInvite { code: String },

    /// Application-level keepalive (optional; the client may send periodically).
    Ping,
    /// Page through this authenticated owner's already-received market ledger.
    RequestTransactions {
        #[serde(default)]
        before: Option<u64>,
        request_id: u32,
    },
}

/// One of the player's own resting limit orders.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct OrderView {
    pub id: u64,
    pub side: Side,
    pub commodity: Commodity,
    pub units: u32,
    pub limit_price: f64,
}

/// A standing price the player reads off the (lagged) hub ticker.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PriceView {
    pub commodity: Commodity,
    pub price: f64,
    /// Delayed ticker view of Sol units available for an immediate corp buy.
    pub available_buy: u32,
    /// Delayed ticker view of Sol demand available for an immediate corp sell.
    pub available_sell: u32,
    /// Units of flow that move this good's price by ~e× — the depth of its book
    /// (§9 thin books for the scarce goods). Static per commodity; the client's
    /// quote preview walks the same curve with it.
    pub depth: f64,
}

/// The hub Exchange as the player sees it — prices **light-delayed** from the
/// hub (§9). `staleness` is how old the ticker is (the hub→command-center light
/// delay); execution still happens at the true current price, so the displayed
/// price is only a guide.
#[derive(Debug, Clone, Serialize)]
pub struct MarketView {
    pub prices: Vec<PriceView>,
    pub staleness: f64,
}

/// One commodity holding in the player's wallet.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct InvSlot {
    pub commodity: Commodity,
    pub units: u32,
}

/// The player's last-arrived Market Hub account report: treasury, Market
/// Warehouse holdings, and resting limit orders share the ticker's light delay.
#[derive(Debug, Clone, Serialize)]
pub struct WalletView {
    /// Legacy save with no report history: unknown, not an empty live account.
    #[serde(default)]
    pub report_pending: bool,
    pub credits: f64,
    /// Equity / net worth, from the slow valuation close (§9).
    pub valuation: f64,
    /// §TCA: goods held at the WORMHOLE HUB — the only stock the Exchange trades
    /// against, and one of exactly two places a corporation's goods can sit (the
    /// other being each owned system's stockpile). Owner-only.
    pub warehouse: Vec<InvSlot>,
    pub orders: Vec<OrderView>,
    /// Total Fuel across all owned systems' stockpiles — the fleet's operating
    /// reserve (§step1 part 2). Owner-only (summed from owned systems only).
    pub fuel_total: f64,
}

/// §TCA Phase 2: the viewer's own CHARTER STANDING with the Authority, and the
/// band it derives. OWNER-ONLY — your legal standing is between you and the
/// Market Hub; rivals learn of your offenses only from the PUBLIC citations
/// that travel at lightspeed, never by reading your record.
#[derive(Debug, Clone, Serialize)]
pub struct CharterView {
    pub standing: f64,
    pub max_standing: f64,
    /// The derived band ("good_standing" … "proscribed").
    pub status: sim::CharterStatus,
    /// Human title of the band, for the status chip.
    pub title: &'static str,
    // (§perf Part B: the static band LADDER moved to Welcome — it is a constant
    // table and had been re-sent inside every 10 Hz View.)
    /// Freight-fee multiplier currently applied (1.0 in good standing).
    pub tariff_mult: f64,
    /// Exchange penalty fee currently applied, as a fraction of trade value
    /// (0.0 in good standing — good-standing players pay nothing).
    pub market_penalty_frac: f64,
    /// Credits per standing point to buy back through reinstatement.
    pub reinstate_cost_per_point: f64,
    /// §standing-ladder (R3): full standing, where buying back stops; grades
    /// lie above it, to `max_standing`.
    pub full_standing: f64,
    pub grade: sim::tca::CharterGrade,
    pub grade_title: &'static str,
    /// The next grade and the standing it begins at.
    pub next_grade: Option<NextGradeView>,
    /// Standing earned past full today, against `earn_per_day`.
    pub earned_today: f64,
    pub earn_per_day: f64,
    /// Units per destination per departure the Authority lifts at this grade.
    pub shipment_cap: u32,
    /// §season (v75): the levy on this corporation's Hub trades (a fraction),
    /// on top of `market_penalty_frac`, and the squeeze it has heard of.
    pub levy_frac: f64,
    pub squeeze: SqueezeView,
    /// §licences (v86): every hull that needs the Authority's licence, as
    /// heard at home: held once the receipt's light has arrived; otherwise
    /// its price at the standing heard (a capital's at Commissioned) and why
    /// it isn't for sale, if it isn't.
    pub licences: Vec<LicenceView>,
    /// §claims (v90): the Authority's claims on new colonies, as heard.
    pub claims: ClaimsView,
}

/// §claims (v90): the claims a corporation holds, as heard at home, and what
/// the next one costs: a system's `SystemInfo.claim_cost` × `mult` (×1.6 for
/// each colony and claim held, the standing tariff). `at_war`: none needed.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ClaimsView {
    pub held: Vec<ClaimHeldView>,
    pub mult: f64,
    pub at_war: bool,
    /// The charter is Revoked or Proscribed: none for sale.
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimHeldView {
    pub system: EntityId,
    pub lapses_at: f64,
}

impl ClaimsView {
    pub fn heard(world: &sim::World, player: PlayerId, standing: f64, now: f64) -> Self {
        let Some(corp) = world.players.get(&player) else { return Self::default() };
        let receipt = sim::transit::delay(world.hub, corp.command_center, world.config.c);
        let held: Vec<ClaimHeldView> = corp.claims.iter()
            .filter(|(_, c)| c.bought_at + receipt <= now)
            .map(|(&system, c)| ClaimHeldView { system, lapses_at: c.lapses_at })
            .collect();
        let mult = sim::claims::price(1.0, world.colonies_beyond_home(player), held.len(), standing);
        Self { held, mult, at_war: world.is_rebel(player),
            closed: sim::tca::charter_status(standing).exchange_closed() }
    }
}

/// §raid-course (v86): has the news of the privateer settling over the
/// colony's world (sim `FoundingRaid.arrived_at`) reached `player`'s command?
/// The tutorial's "Raider over the colony" waits for it.
pub fn raid_over_heard(world: &sim::World, player: PlayerId, now: f64) -> bool {
    let Some(corp) = world.players.get(&player) else { return false };
    let home = corp.home_system.and_then(|id| world.systems.iter().find(|s| s.id == id)).map(|s| s.pos);
    corp.founding.raid.as_ref().and_then(|r| r.arrived_at).zip(home)
        .is_some_and(|(at, home)| at + sim::transit::delay(home, corp.command_center, world.config.c) <= now)
}

/// §licences (v86): one hull's licence, as this corporation has heard it.
#[derive(Debug, Clone, Serialize)]
pub struct LicenceView {
    pub hull: ShipKind,
    pub held: bool,
    pub price: f64,
    /// Why it isn't for sale (`needs_research`, `needs_commissioned`,
    /// `charter_closed`, `at_war`); absent when it is, or is held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<sim::licence::LicenceRefusal>,
}

impl LicenceView {
    /// Every licensed hull for `player`, at the standing it has heard.
    pub fn heard(world: &sim::World, player: PlayerId, standing: f64, now: f64) -> Vec<Self> {
        let Some(corp) = world.players.get(&player) else { return Vec::new() };
        // The Authority sells at the Hub: a purchase is news when its light
        // reaches headquarters.
        let receipt = sim::transit::delay(world.hub, corp.command_center, world.config.c);
        sim::licence::licensed_hulls().into_iter().map(|hull| {
            let held = corp.licences.get(&hull).is_some_and(|at| at + receipt <= now);
            let refusal = sim::licence::offer(hull, standing, world.hull_researched(player, hull), held, world.is_rebel(player))
                .err().filter(|r| !matches!(r, sim::licence::LicenceRefusal::AlreadyHeld));
            LicenceView { hull, held, price: sim::licence::shown_price(hull, standing), refusal }
        }).collect()
    }
}

/// §season (v75): the Authority's demands on one corporation, as heard.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SqueezeView {
    /// The base levy at the grip heard; `levy_frac` adds this corp's rises.
    pub base: f64,
    pub rise: Option<LevyRiseView>,
    pub embargoes: Vec<EmbargoView>,
    pub requisitions: Vec<RequisitionView>,
    /// Requisitions refused this season (three bring a squadron).
    pub refusals: u32,
    pub levy_paid: f64,
    pub enforced_until: Option<f64>,
    /// §season phase 3: this corporation's Resolve, 0–1, and whether that
    /// makes it a loyalist.
    pub resolve: f64,
    pub loyal: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LevyRiseView {
    pub points: f64,
    pub commodity: Commodity,
    pub trade: f64,
    /// Accepted by default after this.
    pub due: f64,
    /// What refusing would owe to lift the embargo.
    pub arrears: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbargoView {
    pub commodity: Commodity,
    pub arrears: f64,
    pub since: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequisitionView {
    pub id: u64,
    pub demand: sim::season::Demand,
    /// Refused by default after this.
    pub due: f64,
}

impl SqueezeView {
    /// What `player` at `cc` has heard of its squeeze by `now`: demands once
    /// their light from the Hub has arrived.
    pub fn heard(world: &sim::World, player: PlayerId, cc: Vec2) -> SqueezeView {
        let lag = sim::transit::delay(world.hub, cc, world.config.c);
        let now = world.time;
        let grip = world.season.census_for(cc, world.hub, world.config.c, now).map_or(sim::season::Grip::Charter, |c| c.grip);
        let Some(sq) = world.season.squeeze.get(&player) else {
            return SqueezeView { base: sim::season::LEVY_BY_GRIP[grip as usize], loyal: true, ..Default::default() };
        };
        SqueezeView {
            base: sim::season::LEVY_BY_GRIP[grip as usize],
            rise: sq.rise.as_ref().filter(|r| r.at + lag <= now).map(|r| LevyRiseView {
                points: r.points, commodity: r.commodity, trade: r.trade, due: r.due,
                arrears: (r.trade * sim::season::ARREARS_FRAC).round(),
            }),
            embargoes: sq.embargoes.iter().map(|(c, e)| EmbargoView { commodity: *c, arrears: e.arrears, since: e.since }).collect(),
            requisitions: sq.requisitions.iter().filter(|r| r.at + lag <= now)
                .map(|r| RequisitionView { id: r.id, demand: r.demand, due: r.due }).collect(),
            refusals: sq.refusals,
            levy_paid: sq.levy_paid,
            enforced_until: (sq.enforced_until > now).then_some(sq.enforced_until),
            resolve: sq.resolve,
            loyal: world.loyal(player),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NextGradeView {
    pub title: &'static str,
    pub at: f64,
}

/// §leave (R4): the owner's leave, current or last, and its fixed terms.
#[derive(Debug, Clone, Serialize)]
pub struct LeaveView {
    /// On leave now.
    pub on: bool,
    /// When the current leave ends by itself.
    pub until: Option<f64>,
    /// The earliest early return from the current leave.
    pub earliest_return: Option<f64>,
    /// When a new leave may start (0 if never taken).
    pub next_start: f64,
    pub min_secs: f64,
    pub max_secs: f64,
    pub cooldown_secs: f64,
}

impl LeaveView {
    pub fn of(leave: &sim::leave::Leave, now: f64) -> Self {
        let on = leave.active(now);
        LeaveView {
            on,
            until: on.then_some(leave.until).flatten(),
            earliest_return: on.then(|| leave.earliest_return()).flatten(),
            next_start: leave.next_start(),
            min_secs: sim::leave::LEAVE_MIN_S,
            max_secs: sim::leave::LEAVE_MAX_S,
            cooldown_secs: sim::leave::LEAVE_COOLDOWN_S,
        }
    }
}

/// Owner-only opening programme. This is a projection of sim state, never a
/// client-side checklist; reconnects resume at the exact milestone.
#[derive(Debug, Clone, Serialize)]
pub struct FoundingView {
    pub stage: sim::FoundingStage,
    pub local_opening: bool,
    pub recovery_site: Option<EntityId>,
    pub recovered: bool,
    pub export_dispatched: bool,
    /// §export-follow: when the first shipment left its dock (sim time), for
    /// the client's follow clock; None until it does.
    pub export_departed_at: Option<f64>,
    pub protected: bool,
    pub protection_min_until: f64,
    pub protection_max_until: f64,
    pub expansion_unlocked: bool,
    pub interceptor: Option<EntityId>,
    pub privateer: Option<EntityId>,
    pub bounty_received: bool,
    /// Opening exports whose sale receipts have reached the command center.
    /// Market-Hub truth stays hidden until the ordinary receipt light arrives.
    pub opening_exports: Vec<Commodity>,
    /// The two nearby prospects assigned to the opening survey chapter. Their
    /// ids are objectives, not survey results: composition/geology still arrive
    /// only through the ordinary served system picture.
    pub survey_candidates: Vec<EntityId>,
    /// §founding R3: the Authority already replaced the starter Freighter.
    pub freighter_replaced: bool,
    /// §founding R6: charter stipends paid so far, in milestone order.
    pub stipends: Vec<StipendView>,
    /// §founding R7: a hull that turned back from the recovery site because of
    /// a threat, once that report has reached command.
    pub turned_back: Option<EntityId>,
    /// §belts: the charted belt the barge lesson points at. It is always in
    /// a system the corporation surveyed or owns.
    pub belt_target: Option<BeltTargetView>,
    /// §charter-v5: command has seen the raiding privateer (from then on the
    /// colony tracks it).
    pub raid_seen: bool,
    /// §charter-v5: the report of what the raid took has reached command.
    pub raided: bool,
    /// §raid-course (Nov 8 2026): the privateer's arrival over the colony's
    /// world (sim `FoundingRaid.arrived_at`, when it starts stripping the
    /// stockpile) has reached command. Until then it is still closing.
    pub raid_over: bool,
    /// §watch-the-pirate (v88, Peter, Nov 9 2026): after its raid, the
    /// privateer has dropped off the colony's sensors (sim
    /// `founding.raid_lost_at`), as heard. The guide watches it until then.
    pub raid_lost: bool,
    /// §charter-v4: the charter's land grant has opened the rest of the home
    /// system ("Expand mining operations"); until then only the landing world.
    pub land_grant_open: bool,
    /// §charter-gates: the structures the charter has yet to open, each with
    /// the step that opens it (`founding::charter_step`). Empty once open.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub charter_gates: BTreeMap<&'static str, usize>,
    /// §charter-v4: the privateer den's system. §den-retreat (Nov 12 2026):
    /// once the hunt begins, since he went home to it after his raid (or once
    /// his kill's report, from his log, if he died first).
    pub den: Option<EntityId>,
    /// §charter-v4: the report that the den fell has reached command.
    pub den_cleared: bool,
    /// §vendetta: the pirate family this founder crossed: the raiding
    /// nephew, his uncle and his brother.
    pub pirates: Option<PirateFamilyView>,
    /// §traders: who flies this founder's free trader (`teo`, `ines`,
    /// `mags` or `rafi`): its name, face and voice in the client.
    pub trader: &'static str,
    /// §charter-advisor (v87): the Authority officer dealt to guide this
    /// charter, 0–2 (`veteran`, `lieutenant`, `frontier` in the client).
    /// Computed, never stored ([`crate::game_loop::charter_advisor`]).
    pub advisor: u8,
    /// §privateer-hunt: where the privateer went to ground, from the raid's
    /// report until its kill's report: the hunt's "last seen" marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privateer_last_seen: Option<Vec2>,
    /// §vendetta: the uncle's revenge, once his oath has reached command.
    pub vendetta: Option<VendettaView>,
    /// §charter-crates: the crates still sealed, in charter order: released
    /// ones wait to be collected; the rest are what the builds ahead can
    /// count on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub crates: Vec<CharterCrateView>,
    /// §stash: what the raid took, in the den until the first freighter
    /// loads it, then on its way home. Absent until the raid's report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stash: Option<StashView>,
    /// §stash: the founder's fleets the den's dying blast left damaged, until
    /// each is repaired.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub den_wounded: Vec<EntityId>,
}

/// §stash (Nov 2 2026): the den's stash. §den-stores (Nov 5 2026): the
/// raid's take and the den's stores, two Tiny Freighter loads.
#[derive(Debug, Clone, Serialize)]
pub struct StashView {
    /// All of it not home yet: at the den or aboard.
    pub goods: BTreeMap<Commodity, u32>,
    /// What still waits at the den for a freighter.
    pub left: BTreeMap<Commodity, u32>,
    /// The freighter bringing a load home, once it has loaded.
    pub fleet: Option<EntityId>,
    /// It reached home, or its freighter was lost on the way.
    pub home: bool,
}

/// §player-styles: the daily loot cap, as its owner sees it.
#[derive(Debug, Clone, Serialize)]
pub struct RaidLossView {
    /// Units raids and blockades took over the last day.
    pub taken: u32,
    /// The day's limit; None while no cap applies (three days without an order).
    pub cap: Option<u32>,
}

/// §charter-crates: one sealed charter crate.
#[derive(Debug, Clone, Serialize)]
pub struct CharterCrateView {
    pub id: sim::CharterCrate,
    /// Its step has come: it can be collected.
    pub released: bool,
    /// The charter step that releases it.
    pub step: usize,
    pub goods: BTreeMap<Commodity, u32>,
}

/// §vendetta: a founder's pirate family (`docs/pirate-vendetta.md`). The
/// family's personality (`showboat`, `hothead`, `collector`, `scrapper` or
/// `fatalist`) picks its portraits and lines; the names are full names.
#[derive(Debug, Clone, Serialize)]
pub struct PirateFamilyView {
    pub family: &'static str,
    pub surname: &'static str,
    pub nephew: String,
    pub uncle: String,
    pub brother: String,
}

/// §vendetta: the uncle's revenge as command knows it.
#[derive(Debug, Clone, Serialize)]
pub struct VendettaView {
    /// When his pack sets out for the colony, once it is raised (step 19).
    pub depart_at: Option<f64>,
    /// His pack, once it is in this viewer's picture.
    pub fleet: Option<EntityId>,
    /// He is over the colony.
    pub arrived: bool,
    /// How it ended, once its report reached command: true when he died.
    pub won: Option<bool>,
    /// §gun-platform: he sacked the colony: game over until a restart.
    pub fallen: bool,
    /// §vendetta-campaign: the latest he sets out, online or not (his
    /// countdown runs only while the founder is online).
    pub deadline_at: Option<f64>,
    /// §vendetta-campaign: his strikes on the founder's expanded mining.
    pub strikes: Vec<StrikeView>,
}

/// §vendetta-campaign: one strike as command knows it.
#[derive(Debug, Clone, Serialize)]
pub struct StrikeView {
    pub kind: sim::founding::StrikeKind,
    /// The mine's world, or the barge's belt.
    pub system: EntityId,
    pub body: u32,
    /// Its raiders, once they are in this viewer's picture.
    pub fleet: Option<EntityId>,
    /// Over the mine's world, on the light of it.
    pub over: bool,
    /// Gone home, called off, or killed.
    pub done: bool,
    /// §gun-platform: the freighter a freighter strike hunts.
    pub target: Option<EntityId>,
    /// §gun-platform: what a mine strike wrecked, in order (structure slugs).
    pub wrecked: Vec<&'static str>,
}

/// §belts: a belt by its system and its body id within that system.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BeltTargetView {
    pub system: EntityId,
    pub body: u32,
}

/// §merchant: the traveling merchant serving the viewer's home. Owner-only;
/// the home is beside the command center, so its stall needs no light delay.
#[derive(Debug, Clone, Serialize)]
pub struct MerchantView {
    pub fleet: EntityId,
    pub system: EntityId,
    pub phase: sim::merchant::MerchantPhase,
    pub leave_by: f64,
    pub markup: f64,
    /// What it sells: stock left, its price and the Hub mid it set out with.
    pub goods: Vec<MerchantGoodView>,
    /// The one good it buys (§sell-alloys, Nov 11 2026: Alloys), what it pays
    /// a unit and how many more it will take.
    pub buys: Commodity,
    pub buy_price: f64,
    pub buy_room: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MerchantGoodView {
    pub commodity: Commodity,
    pub stock: u32,
    pub price: f64,
    pub hub_mid: f64,
    /// §shipbuilding-overhaul: the next unit's featured price, if this is the
    /// trader's one discount (the Interceptor's Fission Core).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured_price: Option<f64>,
}

impl MerchantView {
    pub fn of(visit: &sim::merchant::MerchantVisit) -> Self {
        Self {
            fleet: visit.fleet,
            system: visit.system,
            phase: visit.phase,
            leave_by: visit.leave_by,
            markup: visit.markup,
            goods: visit.stock.iter().map(|(&commodity, &stock)| MerchantGoodView {
                commodity, stock,
                price: visit.sell_price(commodity).unwrap_or(0.0),
                hub_mid: visit.mids.get(&commodity).copied().unwrap_or(0.0),
                featured_price: visit.featured_price(commodity),
            }).collect(),
            buys: sim::merchant::BUYS,
            buy_price: visit.buy_price(sim::merchant::BUYS).unwrap_or(0.0),
            buy_room: visit.room(),
        }
    }
}

/// §founding R6: one paid charter milestone stipend.
#[derive(Debug, Clone, Serialize)]
pub struct StipendView {
    pub stage: sim::FoundingStage,
    pub title: &'static str,
    pub credits: f64,
}

/// §founding R10: the charter ledger — the next goals after founding.
#[derive(Debug, Clone, Serialize)]
pub struct CharterLedgerView {
    pub done: u32,
    pub total: u32,
    /// The next unfinished goals, in ledger order (at most three).
    pub goals: Vec<LedgerGoalView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerGoalView {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub metric: sim::charter::LedgerMetric,
    pub progress: f64,
    pub target: f64,
    /// §standing-ladder (R3): what the goal pays: standing, and a research
    /// dossier as a fraction of a programme.
    pub standing: f64,
    pub research: f64,
}

/// §TCA: one entry of an Authority freighter's MANIFEST, as a viewer may read it.
/// The hull broadcasts under the Convention, but the manifest is TWO-TIER, per
/// entry: the entry's OWNER always sees their own lot (it is their property), and
/// anyone else sees it only from inside sensor range — the same rule that governs
/// a convoy's cargo. So a rival watching a freighter go by learns nothing about
/// who is shipping what until they get close enough to look.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ManifestEntryView {
    pub owner: PlayerId,
    pub commodity: Commodity,
    pub units: u32,
    pub direction: sim::ShipmentDir,
    /// True when this entry is the viewer's own lot.
    pub mine: bool,
}

/// §TCA: one of the viewer's freight shipments — queued for a departure or
/// already aboard an Authority freighter. OWNER-ONLY: a player sees only their
/// own lots, never anyone else's.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ShipmentView {
    pub id: u64,
    /// The destination (outbound) or origin (inbound) system.
    pub system: EntityId,
    pub commodity: Commodity,
    pub units: u32,
    pub direction: sim::ShipmentDir,
    pub sell_on_arrival: bool,
    pub fee_paid: f64,
    pub booked_at: f64,
    /// `false` = still queued at the Market Hub; `true` = aboard a freighter.
    pub aboard: bool,
}

/// §TCA: the Authority's freight TERMS for one of the viewer's owned systems —
/// everything the client needs to price and time a booking BEFORE committing.
/// Deterministic: the departure phase and the freighter's constant cruise are
/// pure functions of the config, so these are exact, not estimates.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FreightTermsView {
    pub system: EntityId,
    /// Market Hub → system distance (sim units).
    pub distance: f64,
    /// Max units this corp may load to this destination per departure. Uniform
    /// across destinations — nothing the colony builds changes the Authority's terms.
    pub cap: u32,
    /// Flight time one way (seconds) — add to a departure for the outbound ETA.
    pub secs_out: f64,
    /// Flight time out AND back (seconds) — an inbound lot's total after departure.
    pub secs_round: f64,
}

/// §TCA: the Market Hub freight desk — the timetable, the fee formula's inputs,
/// the viewer's own shipment queue. Owner-only.
#[derive(Debug, Clone, Serialize)]
pub struct FreightView {
    /// Sim-time of the next scheduled departure (exact).
    pub next_departure: f64,
    /// Seconds between departures.
    pub period: f64,
    /// Fee = units × (price × `fee_frac` + distance × `fee_per_unit_dist`). Exposed
    /// as inputs (not a per-commodity table) so the client prices any lot live off
    /// the ticker; `freight_view_terms_price_a_lot_exactly` guards the contract.
    pub fee_frac: f64,
    pub fee_per_unit_dist: f64,
    /// Terms for each system the viewer currently owns (the valid destinations).
    pub terms: Vec<FreightTermsView>,
    /// The viewer's own lots, queued and aboard.
    pub shipments: Vec<ShipmentView>,
}

/// Which side of a raid the recipient is on.
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Attacker,
    Defender,
}

/// A delayed report of a raid outcome (§8), tailored to the recipient. Delivered
/// only once the light of the event has reached the player's command center, so
/// attacker and defender may receive it at different times. §FLEETS Part 2: now
/// a composition-vs-composition report — `attacker_kind`/`target_kind` are the
/// flagships, and `*_losses` carry the per-kind ships each side lost.
#[derive(Debug, Clone, Serialize)]
pub struct RaidReport {
    /// §battle-aftermath: stable id shared with the RETAINED copy of this
    /// report (`View.battle_reports`) — the news toast and the map marker /
    /// results panel can point at the same battle.
    pub report_id: u64,
    /// Exact engagement id, carried only with this already-arrived report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battle_id: Option<EntityId>,
    /// Frozen own-side survivors/rewards, gated with the result (never rivals).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aftermath: Option<sim::combat::aftermath::BattleAftermath>,
    pub outcome: RaidOutcome,
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub attacker_ship: EntityId,
    pub target_ship: EntityId,
    pub attacker_kind: ShipKind,
    pub target_kind: ShipKind,
    pub pos: Vec2,
    /// Sim time at which the battle resolved.
    pub at_time: f64,
    /// How long ago (light delay, seconds) — you are learning this stale news.
    pub age: f64,
    /// The recipient's side.
    pub you: Role,
    /// Per-kind ships the attacker lost over the engagement.
    pub attacker_losses: Vec<CompCount>,
    /// Per-kind ships the defender (target side) lost over the engagement.
    pub target_losses: Vec<CompCount>,
}

/// §battle-aftermath: a RETAINED concluded-battle report, as one participant
/// learned it — powers the aftermath map marker and the battle-results panel.
/// Owner-only by construction (built per player from their retained journal).
#[derive(Debug, Clone, Serialize)]
pub struct BattleReportView {
    /// Stable id (shared with the transient `Report` news toast for the same
    /// battle, and usable by the timeline to open the same results).
    pub id: u64,
    /// None on legacy reports/no-contact escapes: keep them readable, but do
    /// not guess a replay or hide a marker by matching coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battle_id: Option<EntityId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aftermath: Option<sim::combat::aftermath::BattleAftermath>,
    pub pos: Vec2,
    /// Sim-time the battle CONCLUDED (what happened, when).
    pub at_time: f64,
    /// Sim-time this player's conclusion light ARRIVED (when you learned).
    pub learned_at: f64,
    /// The recipient's side in it.
    pub you: Role,
    pub attacker_kind: ShipKind,
    pub target_kind: ShipKind,
    pub outcome: RaidOutcome,
    /// Composition-vs-composition per-kind losses, as this side learned them.
    pub attacker_losses: Vec<CompCount>,
    pub target_losses: Vec<CompCount>,
}

/// §contestable-territory Part 2: a RETAINED capture report, as one participant
/// learned it — powers the capture aftermath marker + results panel. Owner-only
/// by construction (per participant). `captor` = you took the system; else you
/// lost it. `plunder` is the seized stockpile the captor gained / the old owner
/// lost (the defender's report itemizes it).
#[derive(Debug, Clone, Serialize)]
pub struct CaptureReportView {
    pub id: u64,
    pub pos: Vec2,
    /// Sim-time the system FLIPPED.
    pub at_time: f64,
    /// Sim-time THIS player's light arrived (when you learned).
    pub learned_at: f64,
    pub captor: bool,
    pub plunder: Vec<StockSlot>,
}

/// A PROJECTED engagement estimate (§FLEETS Part 3), computed by running the
/// SAME shared Lanchester attrition forward on the observer's own view data. It
/// is honest about staleness: `composition_age` is how old the target sighting
/// is, `defenses_age` how old the scout snapshot of its fortifications is, and
/// `target_known = false` means the target was OUT of sensor coverage so the
/// projection assumed a typical warfleet of the estimated bucket size (never the
/// true count). Deterministic; never touches authoritative state.
#[derive(Debug, Clone, Serialize)]
pub struct EngagementEstimate {
    pub attacker: EntityId,
    pub target: EntityId,
    /// Projected per-kind losses on each side.
    pub own_losses: Vec<CompCount>,
    pub target_losses: Vec<CompCount>,
    /// Projected survivors on each side.
    pub own_survivors: Vec<CompCount>,
    pub target_survivors: Vec<CompCount>,
    /// True if the target's exact composition was known (in sensor coverage);
    /// false if the projection used the bucket-midpoint typical-hull assumption.
    pub target_known: bool,
    /// The target's estimated-size bucket (always available).
    pub target_count_class: CountClass,
    /// Age of the target sighting the estimate is built on (seconds).
    pub composition_age: f64,
    /// Age of the scouted-defenses snapshot folded in, if any (seconds).
    pub defenses_age: Option<f64>,
    /// Scouted platform tiers folded into the target, if a snapshot covered it.
    pub platform_tiers: Option<u32>,
    /// §orbital-defense: the scouted station level folded in, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station_level: Option<u32>,
    /// §tactical T4: the Monte Carlo readout — attacker-favorable fraction over
    /// `runs` rollouts of the REAL engine on derived seeds ("68% favorable").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub win_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs: Option<u32>,
    /// Per-kind 25th–75th percentile loss bands ("expected losses 4–7
    /// Corvettes"). Predictive Plots research widens the DISPLAY of these —
    /// it never invents math.
    #[serde(default)]
    pub own_loss_bands: Vec<LossRange>,
    #[serde(default)]
    pub target_loss_bands: Vec<LossRange>,
}

/// §tactical T4: one per-kind loss band (25th–75th percentile of the rollouts).
#[derive(Debug, Clone, Serialize)]
pub struct LossRange {
    pub kind: ShipKind,
    pub lo: u32,
    pub hi: u32,
}

/// Severity of a check-in timeline entry — drives the client's colour/icon.
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineSeverity {
    Good,
    Bad,
    Warn,
    Info,
}

/// One entry in the player's check-in timeline (§16, Layer 3): a discrete thing
/// that became OBSERVABLE to them at `at_time` (their own clock — own economy is
/// instant, distant battles/rival claims arrive light-delayed). The server
/// composes the human-readable `text`; the client lists entries newest-first.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct TimelineEntry {
    /// Sim-time the news became observable to this player.
    pub at_time: f64,
    pub severity: TimelineSeverity,
    pub text: String,
    /// §log (v71): stable per entry, for read marks. 0 on entries saved before.
    #[serde(default, skip_serializing_if = "is_zero_id")]
    pub id: u64,
    /// §log: what the entry is about (the Log's filters, glyphs and reports).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<LogKind>,
    /// §log: entries about one real-world thing (a raid on a home, a
    /// blockade, a construction) share a group; the client shows an episode
    /// as one row, split at its terminal phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<LogPhase>,
    /// §log: what the entry names, for links.
    #[serde(default, skip_serializing_if = "LogRefs::is_empty")]
    pub refs: LogRefs,
    /// §log: the numbers behind the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Box<LogData>>,
}

fn is_zero_id(n: &u64) -> bool {
    *n == 0
}

/// §log (v71): what a timeline entry is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    /// Pirate raids on a home, the founding raid and pirate dens.
    Raid,
    Blockade,
    Battle,
    /// Captures, landings and claims of territory.
    Capture,
    Trade,
    Freight,
    /// The traveling merchant.
    Trader,
    Build,
    /// Production stopping and starting, power and food.
    Production,
    Research,
    Survey,
    Signal,
    /// Exotic nodes and rival claims.
    Territory,
    Operation,
    Diplomacy,
    Syndicate,
    /// Fleet status: fuel, rescue, repairs, jumps, arrivals.
    Fleet,
    /// Refused orders, builds and trades.
    Refusal,
    /// Authority stipends, citations and enforcement.
    Charter,
    Vendetta,
    Medal,
    /// Leave and other corporation administration.
    Admin,
}

/// §log (v71): where an entry's episode stands. Terminal phases end it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogPhase {
    Inbound,
    Attacking,
    Raiding,
    Ended,
    Contested,
    Held,
    Lifted,
    Offered,
    Active,
    Completed,
    Failed,
    Expired,
    Started,
    Done,
}

/// §log (v71): the things an entry names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct LogRefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fleet: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<sim::OperationId>,
    /// The battle record a combat line reports (opens its replay).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub battle: Option<EntityId>,
}

impl LogRefs {
    pub fn is_empty(&self) -> bool {
        self.system.is_none() && self.fleet.is_none() && self.operation.is_none() && self.battle.is_none()
    }
}

/// §log (v71): the numbers an entry reports, from this player's side.
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct LogData {
    /// Goods the event moved for this player: negative is lost.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub goods: BTreeMap<Commodity, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ships: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<f64>,
    /// How many like events this entry stands for (merged at the source).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// The ids those events concern ("O12793"…).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// Sim-time something is expected to arrive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<f64>,
    /// Seconds it lasted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// One resource deposit on a system, as the client sees it. §explore: NO LONGER
/// public — the exact geology is CORP KNOWLEDGE (surveyed-or-owner), shipped
/// per-player in [`SystemStateView::deposits`]; the public spectral read is the
/// richness `band` on [`SystemInfo`].
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DepositView {
    pub resource: Commodity,
    /// Content rate in BULK-equivalent units per second at full extraction — the
    /// survey's `×` scale. A dense ore (§ore-density) leaves the ground in
    /// fewer, richer units than this.
    pub richness: f64,
    /// Remaining reserves; `null` = renewable.
    pub reserves: Option<f64>,
}

/// A star system as static PUBLIC geography: position, name, and the richness
/// BAND (§explore R1 — the free spectral read; Poor/Fair/Rich by galaxy-wide
/// terciles, same for everyone, never changes). Sent once at join. The exact
/// geology is per-corp knowledge in [`SystemStateView::deposits`]; dynamic state
/// (owner, stockpile) is light-gated there too.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    pub id: EntityId,
    pub pos: Vec2,
    pub name: String,
    /// §explore: the public richness band slug — "poor" | "fair" | "rich".
    pub band: &'static str,
    /// §claims (v90): the base price of the Authority's claim on it; the
    /// asking price is this × `ClaimsView.mult`.
    pub claim_cost: f64,
    /// §claims: a home site held for a new charter. No claim is sold on it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reserved: bool,
    /// §node: the star is exotic (`World::exotic_bonus`). The client's star
    /// art follows this rather than its own roll of the id, since a home is
    /// never exotic.
    pub exotic: bool,
}

/// Static public deep-space terrain. Geometry is the exact rotated ellipse used
/// by the sim; explicit multipliers make client previews/readouts rule-derived
/// instead of duplicating balance constants.
#[derive(Debug, Clone, Serialize)]
pub struct NebulaInfo {
    pub id: u32,
    pub kind: sim::NebulaKind,
    pub name: String,
    pub center: Vec2,
    pub radius_x: f64,
    pub radius_y: f64,
    pub rotation: f64,
    pub signature_mult: f64,
    pub sensor_mult: f64,
    pub jump_range_mult: f64,
}

/// Static galaxy geography, sent once at join. Never changes during a session
/// (systems don't move), so it doesn't need to be in the per-tick stream.
#[derive(Debug, Clone, Serialize)]
pub struct GalaxyInfo {
    /// Opaque identity of this game, not its reproducible map seed. Persists
    /// across saved-game restarts; a fresh galaxy gets a fresh identity.
    pub instance_id: String,
    pub hub: Vec2,
    pub radius: f64,
    /// Public deep-space terrain; static for the life of the galaxy.
    pub nebulas: Vec<NebulaInfo>,
    /// Speed of light (sim units / s) — lets the client annotate light-delays.
    pub c: f64,
    /// Public jump-drive geometry and timing. These mirror the sim constants so
    /// the client can preview an honest, served-picture estimate; the sim still
    /// decides legality against truth when the command arrives.
    pub jump_range: f64,
    pub jump_spool_s: f64,
    pub hyperlimit: f64,
    /// Full detection radius of the command center and Raider pickets.
    pub sensor_range: f64,
    /// Raider cruise speed (sim units / s) — lets the client compute a CRUDE,
    /// drifting intercept estimate for a committed raid (rendered as a soft zone).
    pub raider_speed: f64,
    /// Multiplier when a Scout accompanies a Raider-bearing sensor fleet.
    pub scout_sensor_mult: f64,
    /// Short-range multiplier for a Convoy's traffic/threat sensor.
    pub convoy_sensor_mult: f64,
    /// Sensor-array bubble tunables (§buildings step 2b): a tier-N array projects
    /// `base + per_tier · (N−1)` — lets the client draw its own arrays' coverage.
    pub sensor_array_base: f64,
    pub sensor_array_per_tier: f64,
    /// Defense Platform protection radius (§buildings step 2c) — lets the client
    /// draw a subtle ring on the OWNER's own defended systems.
    pub defense_platform_radius: f64,
    /// §economy Part 2 colony tunables — for the owner-only colony readout:
    /// Provisions/s eaten per million population and population capacity per
    /// Habitat tier (millions). `pop_growth_per_s` is retained at zero for
    /// rolling-wire compatibility; demographic growth is physical migration.
    pub provisions_per_million_per_s: f64,
    pub pop_cap_per_habitat_tier: f64,
    pub pop_growth_per_s: f64,
    /// Physical immigration tunables: people per liner and the corporation's
    /// base interval between Hub allocations before appeal/research modifiers.
    pub migrant_cohort_people: u32,
    pub migration_base_interval_s: f64,
    /// §economy Part 6: Sol's standing specialist contract price (credits) —
    /// for the hire panel.
    pub specialist_hire_cost: f64,
    /// §economy Part 3: the Fuel Refinery's converter rate — units of Fuel/s
    /// at tier-throughput 1.0 with full staffing (the basket is 1 Volatile per
    /// Fuel). For the owner-only refining hint until the Part-6 colony panel.
    pub fuel_refinery_rate: f64,
    /// §contestable-territory Part 2: how long (sim seconds) an unbroken,
    /// defense-suppressed siege must run before a colony ship can capture — the
    /// client renders siege progress against it.
    pub siege_secs: f64,
    /// §pirates: the reserved neutral PIRATE faction id (a `PlayerId`), so the
    /// client can label pirate contacts/raids/reports without a name lookup.
    pub pirate_id: PlayerId,
    /// §node: sim-time (seconds) at which every EXOTIC system AWAKENS into a
    /// capturable node — the client telegraphs the countdown from t=0.
    #[serde(default)]
    pub node_awakening_time: f64,
    /// §node: a node's region radius (sim units) — the client draws the holder's
    /// region ring and the "in-region" cue with it.
    #[serde(default)]
    pub node_region_radius: f64,
    pub systems: Vec<SystemInfo>,
    /// What a player can BUILD at an owned system + each recipe's cost/time (§step1).
    /// Static (const recipes), sent once so the client renders costs without re-tx.
    pub build_options: Vec<BuildOptionView>,
    pub industry_catalog: sim::industry::IndustryCatalog,
}

/// A buildable thing and its recipe (§step1 growth sink), for the System-view UI.
/// `key` is a stable identifier the client maps back to a build command.
#[derive(Debug, Clone, Serialize)]
pub struct BuildOptionView {
    pub key: String,
    pub label: String,
    pub costs: Vec<StockSlot>,
    pub build_secs: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub research_prerequisite: Option<&'static str>,
    /// Other programmes whose completed effects unlock the same structure.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub research_alternatives: Vec<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tiers: Vec<StructureTierView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumes_slot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_population: Option<u32>,
    /// §gaia: the environment a world must have (Living Terraces: "gaia").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_environment: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion: Option<ConversionRecipeView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refining_recipes: Vec<ConversionRecipeView>,
    /// §asteroid-quarry: what one tier eats a second from its system's stock,
    /// all or nothing (`[commodity, units/s]`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub upkeep: Vec<(Commodity, f64)>,
    /// §progression: an income structure; its levels follow
    /// `build::PRODUCTION_TIER_COST_MULT` instead of the gentle curve.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub earns: bool,
}

/// Explicit level-specific structure recipe and benefit, sent with Welcome.
#[derive(Debug, Clone, Serialize)]
pub struct StructureTierView {
    pub tier: u32,
    pub label: String,
    pub costs: Vec<StockSlot>,
    pub build_secs: f64,
    pub research_prerequisite: Option<&'static str>,
    pub description: String,
    /// A yard the system must already hold for this level (§orbital-defense).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<YardNeedView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct YardNeedView {
    pub yard: &'static str,
    pub tier: u32,
}

/// §orbital-defense: a station as its owner knows it.
#[derive(Debug, Clone, Serialize)]
pub struct StationView {
    pub level: u32,
    pub name: &'static str,
    /// The body it orbits.
    pub body_id: u32,
    /// One per hardpoint: its fit (empty = the stock beam battery) and its
    /// hull fraction (0 = knocked out).
    pub mounts: Vec<MountView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MountView {
    pub modules: Vec<sim::ModuleKind>,
    pub health: f64,
    /// Knocked out: silent until repaired to a quarter of its hull.
    pub offline: bool,
}

/// Static factory recipe, sent once with Welcome. Fractional inputs are per
/// one base output; actual throughput still depends on served staffing/tier.
#[derive(Debug, Clone, Serialize)]
pub struct ConversionRecipeView {
    pub output: Commodity,
    pub rate: f64,
    pub inputs: Vec<(Commodity, f64)>,
    pub byproducts: Vec<(Commodity, f64)>,
}

/// One commodity in a system's stockpile (whole units), shown only to the owner.
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
pub struct StockSlot {
    pub commodity: Commodity,
    pub units: u32,
}

/// §stores-flows: what a system's economy makes and uses of one good, per
/// minute, from the same report as its stockpile. Owner-only.
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
pub struct FlowView {
    pub commodity: Commodity,
    pub made: f64,
    pub used: f64,
}

/// The BLOCKADE at a system as a participant learns it (§contestable-territory).
/// Only ever populated for the besieger and the owner (fog-safe — see the
/// `SystemStateView.blockade` doc). `by` names the blockading corp; `since` is
/// when hostile fleets first took station; `by_me` marks the viewer as the
/// besieger. Stages: CONTESTED while `held_since` is null (the defense still
/// fights: only a battle), HELD once it is set (defense beaten: logistics cut,
/// stores stripped), a SIEGE while `siege_since` is set.
#[derive(Debug, Clone, Serialize)]
pub struct BlockadeStateView {
    pub by: PlayerId,
    pub since: f64,
    pub by_me: bool,
    /// Sim-time the blockade took hold, or null while contested.
    pub held_since: Option<f64>,
    /// §Part 2 SIEGE: sim-time the capture clock started, or null if it isn't
    /// running (contested, or a home, which is never captured). Progress =
    /// (now − siege_since) / `siege_secs`; capture becomes possible at full.
    pub siege_since: Option<f64>,
    /// The capture clock this system needs: the base stretched by its garrison
    /// and shortened by a battleship-class besieger on station, as reported.
    /// Null only where the game loop hasn't filled it (use
    /// `GalaxyInfo.siege_secs`).
    pub siege_secs: Option<f64>,
    /// False at a corporation's home system, which can be blockaded but never
    /// captured.
    pub capturable: bool,
    /// The besieger's name as the players know it ("Pirates", "the Authority"
    /// for the sentinels).
    pub by_name: String,
    /// While held: the chance the blockade catches a freighter running it, as
    /// reported. For the owner it includes their Blockade Runners research;
    /// the besieger sees it against an unmodified freighter.
    pub catch: f64,
    /// The same, with a Warp Stabilizer on every hull of the runner.
    pub catch_stabilized: f64,
}

/// §ground: the GROUND situation at a system — what a landing would have to
/// beat. Fog-safe, and populated for exactly two viewers: the OWNER (it is their
/// building) and the corp currently BLOCKADING it (their fleet is parked
/// overhead measuring it, and without this reading bombardment would be an
/// invisible mechanic — you could never tell whether the guns were working).
/// Third parties get null.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GroundStateView {
    /// Garrison tiers standing here, summed across bodies.
    pub garrison_tier: u32,
    /// Whether the garrison is drawing its Provisions. An unfed garrison stops
    /// counting entirely — it defends nothing until the colony feeds it.
    pub garrison_fed: bool,
    /// 0..1 — how much of the garrison orbital bombardment currently has pinned.
    /// Bleeds off the moment the guns stop, so it is a window, not a wound.
    pub suppression: f64,
    /// The BREAK-EVEN landing — the even-odds point, not a guarantee. Falls as
    /// suppression rises; 0 means the ground is open.
    pub marines_needed: u32,
    /// §ground G4: the PRE-COMMIT ESTIMATE for the force this viewer actually
    /// has in orbit, sampled from the real ground engine. Present only for a
    /// besieger with marines on station — it is a decision aid for the person
    /// making the decision, and it would be an intel leak for anyone else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing: Option<LandingOddsView>,
    /// §veil: the system's Veil Projector, for the two siege parties (an
    /// aurora is not subtle). `None` without a projector.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veil: Option<VeilStateView>,
}

/// §veil (docs/veil.md): the ionised sky a besieged world can raise.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VeilStateView {
    /// Projector level, 1–3.
    pub level: u32,
    /// Raised right now: guns pin nothing, plunder loads nothing, only
    /// Transports land.
    pub up: bool,
    /// 0..1 of a landing the Veil shreds in the descent at this level.
    pub descent_loss: f64,
    /// Units of the plant's fuel the projector burns a second while raised.
    pub burn_per_s: f64,
    /// The fuel it burns (`None`: a Solar Array, which can't carry it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuel: Option<Commodity>,
    /// Owner's standing order: raise under siege (false = forbidden). Owner-only
    /// meaning; a besieger just sees whether it is up.
    pub raise: bool,
}

/// §ground G4: what a landing with the marines currently in orbit would cost.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LandingOddsView {
    /// Marines this viewer has on station, i.e. what the odds below are for.
    pub marines: u32,
    /// 0..1 chance of taking the ground at the CURRENT suppression.
    pub win: f64,
    /// 0..1 chance if the bombardment stops the moment the men are down. The
    /// GAP between this and `win` is the warning: a landing that only works
    /// while the guns fire lasts exactly as long as you hold the orbit.
    pub win_if_guns_leave: f64,
    /// Mean marines lost, and mean seconds the fight runs.
    pub expected_losses: u32,
    pub expected_secs: f64,
}

/// An owner-only in-progress build at a system (§step1). `key` is what's building;
/// All fields come from the arrived site report. `complete_time` is an estimate
/// (None while waiting/paused), never evidence of completion. Builds also carry their
/// latest earned-work segment so pauses cannot stretch/restart the progress bar.
#[derive(Debug, Clone, Serialize)]
pub struct BuildStateView {
    /// §construction: the sim's job id, so a waiting structure can be cancelled.
    pub id: u64,
    pub key: String,
    /// Ship's original yard from the same delayed report (including legacy jobs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yard: Option<String>,
    /// Absent for waiting structures or legacy jobs with no recorded start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f64>,
    pub complete_time: Option<f64>,
    /// The arrived report says this structure is waiting for its planet's turn.
    /// Clock expiry alone must never promote it to an active build.
    pub queued: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work: Option<BuildWorkView>,
    /// §bodies: the body this job builds on / displays at.
    pub body_id: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildWorkView {
    pub fraction: f64,
    pub per_second: f64,
    pub at_time: f64,
}

/// §scouting (v74): a scout's look at a RIVAL system's stores, Travian's
/// "resources" report, as delivered to the scout's owner and nobody else.
#[derive(Debug, Clone, Serialize)]
pub struct ScoutedStoresView {
    /// Whole units of each good in raiders' reach, dearest first.
    pub reach: Vec<StockSlot>,
    /// The system's Vault room (units), whatever it holds.
    pub vault: u32,
    /// Sim-time of the look; the readout ages from it.
    pub observed_at: f64,
}

/// A stored SCOUT-INTEL snapshot of a RIVAL system's fortifications (§scout
/// part 2), as delivered to the scout's owner — and to nobody else. It is a
/// SNAPSHOT: `observed_at` is when the scout saw it (the client ages it); it is
/// never live and never auto-updates — the rival may have built since.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct IntelView {
    pub defense_tier: u32,
    pub shipyard_tier: u32,
    /// Sim-time of the observation — T₁, the "as of T" the readout ages from
    /// (the ORIGINAL observation, even when relayed by an ally; §syndicates Part 2).
    pub observed_at: f64,
    /// §syndicates Part 2 relay PROVENANCE — present only for ALLY-sourced intel
    /// (`None` for your own direct scout). Who observed it, and the two chain
    /// legs: `relayed_at` = T₂ (the observation's light reached the ally's command
    /// center — the earliest they could relay), `received_at` = T₃ (the relayed
    /// report's light reached YOUR command center). The picture is honestly staler
    /// than the ally's by the inter-command-center distance, and NEVER upgrades to
    /// live truth — aging is always from T₁.
    #[serde(default)]
    pub relayed_by: Option<PlayerId>,
    #[serde(default)]
    pub relayed_at: Option<f64>,
    #[serde(default)]
    pub received_at: Option<f64>,
    /// §pirates: the scouted PIRATE ENCLAVE tier at this system (0 = not an
    /// enclave). When > 0 the site is a pirate base; `defense_tier` above is its
    /// platform-equivalent base defense (what an assault must grind down).
    #[serde(default)]
    pub enclave_tier: u32,
}

/// The DYNAMIC, per-tick, light-gated state of a star system (companion to the
/// static [`SystemInfo`]). `owner` is revealed to rivals only once the claim's
/// light has reached the viewer's command center — the owner sees their own claim
/// instantly (§6). `stockpile` (accumulated production) is private: present only
/// for the owner. No information about a rival's holdings ever leaks.
#[derive(Debug, Clone, Serialize)]
pub struct SystemStateView {
    pub id: EntityId,
    /// Owner-only; the SAME arrived site report as stockpile, never live truth.
    pub industry: Option<sim::industry::SiteIndustry>,
    pub owner: Option<PlayerId>,
    pub stockpile: Option<Vec<StockSlot>>,
    /// §vault (v73): owner-only — the whole units of each good raiders can't
    /// reach (the settlers' rations and what the Vaults hold), from the same
    /// report as `stockpile`. Empty for rivals.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vault: Vec<StockSlot>,
    /// §stores-flows (Nov 5 2026): owner-only — per good, what the economy
    /// makes and uses a minute (production, food, power, upkeep, repairs,
    /// research; never transfers), from the same report. Empty for rivals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flows: Vec<FlowView>,
    /// Owner-only: the SOONEST in-progress build at this system (§step1), if
    /// any. Like `stockpile`, never present for a rival — build state never
    /// leaks. Kept alongside `builds` for the single-job consumers.
    pub build: Option<BuildStateView>,
    /// Owner-only: ALL in-progress builds at this system, ordered by completion
    /// (§build-progress — the sim has always allowed concurrent jobs; the view
    /// used to collapse them to the soonest). Same fog rule as `build`: a rival
    /// always sees an empty list.
    pub builds: Vec<BuildStateView>,
    /// Number of Extractor upgrades built here (visible to all once the system is
    /// known — it's part of the system's observable development, not private intel).
    pub extractor_tier: u32,
    /// Number of Orbital Warehouse tiers built here (§buildings step 2) — owner-only.
    pub orbital_warehouse_tier: u32,
    /// Number of Shipyard upgrades built here (§buildings step 3) — owner-only.
    /// Gates ship construction: Convoy needs ≥ 1, Raider ≥ 2.
    pub shipyard_tier: u32,
    /// Number of Sensor Array upgrades built here (§buildings step 2b) —
    /// owner-only. Projects a standing sensor bubble for the owner.
    pub sensor_tier: u32,
    /// Number of Defense Platform tiers standing here (§buildings step 2c) —
    /// owner-only. A rival learns a platform exists only through engagement
    /// outcomes (delayed battle reports), never through the View.
    pub defense_tier: u32,
    /// Number of Habitat tiers here (§buildings step 3a) — owner-only.
    pub habitat_tier: u32,
    /// §economy Part 2: whether the colony is WELL SUPPLIED — owner-only (rivals
    /// always see false; a rival must never learn whether your colonies are
    /// hungry). Kept under the legacy wire name so the client's amber
    /// supply-trouble tint keeps working; `food_state` below has the full rung.
    pub habitat_fed: bool,
    /// §economy Part 2: the colony's food-ladder rung (slug: `well_supplied` /
    /// `rationing` / `critical` / `no_provisions`) — owner-only; rivals always
    /// see `well_supplied` (the vacuous rung — same fog rule as `habitat_fed`).
    pub food_state: String,
    /// §economy Part 2: colony POPULATION in millions — owner-only; rivals
    /// always see 0 (workforce/economy strength is private intel).
    pub population: f64,
    /// §planetary-identity: environment-adjusted BASE population upkeep —
    /// owner-only; rivals always see 0. Research may reduce the actual draw.
    pub population_upkeep: f64,
    /// §economy Part 4: the RESIDENT SPECIALIST pool — owner-only; rivals
    /// always see an empty map (your talent is private intel).
    pub specialists: BTreeMap<sim::SpecialistKind, u32>,
    /// §bodies: the system's PLANETS AND MOONS — roster public, deposits
    /// survey-gated, per-body owner data owner-only (see [`BodyView`]).
    pub bodies: Vec<BodyView>,
    /// §economy Part 6: every built STRUCTURE (slug → tier) — owner-only;
    /// rivals always see an empty map (the legacy per-kind tier fields above
    /// stay for the map's visual anchors).
    pub structures: BTreeMap<String, u32>,
    /// §economy Part 6: the colony's WORKFORCE — owner-only; None for rivals.
    pub workforce: Option<WorkforceView>,
    /// §economy Part 6: every production line with its RESOLVED factor chain
    /// (the shown-math law: output = base · throughput · staffing · skill ·
    /// food) — owner-only; rivals always see an empty list.
    pub assignments: Vec<AssignmentView>,
    /// §economy: per built-converter idle status for the system-view banner —
    /// owner-only (rivals always see an empty list). serde default keeps old
    /// clients parsing.
    #[serde(default)]
    pub converters: Vec<ConverterStatusView>,
    /// Arrived local refining capability, including a tier-I preview before
    /// construction. Empty for rivals; never filled from current research.
    pub refining_sites: Vec<RefiningSiteView>,
    /// Number of Fuel Refinery tiers here (§buildings step 3b) — owner-only.
    pub refinery_tier: u32,
    /// BLOCKADE state (§contestable-territory Part 1), if hostile fleets hold
    /// station here. Populated for the two PARTICIPANTS only, each from the
    /// light-delayed report of the system that has reached them (the besieger's
    /// own timeline lines arrive at once; the readout rides the light like
    /// everything else). Third parties get `None` here — they observe the fight
    /// via `battles`, and the eventual capture via the light-delayed ownership
    /// change.
    pub blockade: Option<BlockadeStateView>,
    /// §ground: what a landing here would face — owner + besieger only (see
    /// `GroundStateView`); null for everyone else.
    pub ground: Option<GroundStateView>,
    /// §orbital-defense: the system's station, OWNER-ONLY, from the same
    /// arrived site report as the stockpile. Absent without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station: Option<StationView>,
    /// Development slots USED at this system (built tiers + in-progress upgrade
    /// jobs) — owner-only, like `stockpile`; rivals always see 0 (§buildings step 1).
    pub slots_used: u32,
    /// The system's development slot budget — owner-only; rivals always see 0.
    /// (The budget derives from public geology, but exposing it only to the owner
    /// keeps the whole slots readout on one fog rule.)
    pub slots_total: u32,
    /// TOTAL storage capacity (units, summed across commodities; §buildings step 2)
    /// — owner-only; rivals always see 0.
    pub storage_cap: u32,
    /// §research-wiring Bulk Storage: the warehouse multiple (owner-only; 1 for rivals).
    pub storage_mult: f64,
    /// §research-wiring Flash Forges: structures each world builds at once.
    pub structure_slots: u32,
    /// §research-wiring: specialists the system houses (owner-only; 0 for rivals).
    pub specialist_housing: u32,
    /// Units currently stored against that cap — owner-only; rivals always see 0.
    /// (May exceed `storage_cap` for a grandfathered pre-cap stockpile.)
    pub storage_used: u32,
    /// The VIEWER'S own scout-intel snapshot of this (rival) system, if any —
    /// present only once the capture's light has reached the viewer's command
    /// center (§scout part 2). Never present on the viewer's own systems, and a
    /// scouted rival never sees anything here about being scouted.
    pub intel: Option<IntelView>,
    /// §scouting (v74): what the viewer's own scouts last saw of this RIVAL
    /// system's stores, once that light reached its command center. Injected
    /// by the game loop; absent on your own systems.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scouted_stores: Option<ScoutedStoresView>,
    /// True if this system's (light-gated known) owner is a SYNDICATE ally as the
    /// viewer knows it (§syndicates Part 1). Drives the friendly ally tint; does
    /// NOT grant any owner-only data (stockpile/tiers stay private in Part 1).
    #[serde(default)]
    pub ally: bool,
    /// §syndicates Part 3: OWNER-ONLY — an ally GARRISON hosted at THIS system (the
    /// coalition shield you're feeding). Total ally garrison ships stationed here,
    /// and whether their Provisions upkeep is currently covered. `0` = none; rivals
    /// always see 0 (a private detail of your own system).
    #[serde(default)]
    pub ally_garrison_ships: u32,
    #[serde(default)]
    pub ally_garrison_fed: bool,
    /// §node: this system's EXOTIC NODE, if any. Present (light-gated with the
    /// system) whenever the system carries a node — the bonus slug + awakened flag
    /// are PUBLIC (an awakened node is a galaxy-wide landmark, and the exotic star
    /// is already visible to everyone). `None` for ordinary systems.
    #[serde(default)]
    pub node: Option<NodeStateView>,
    /// §explore R2: the EXACT deposit table — present iff the viewer has SURVEYED
    /// this system or OWNS it (survey knowledge is permanent; holding a system is
    /// knowing it). `None` = unsurveyed: the viewer gets only the public band.
    /// Never leaks a rival's survey state (each corp is gated on its OWN set).
    #[serde(default)]
    pub deposits: Option<Vec<DepositView>>,
    /// §explore R3: the survey-gated economic trait slug — present for this
    /// viewer's surveyed systems or holdings, never from rival knowledge. A
    /// Bonus Vein carries its commodity as `bonus_vein:<commodity>`.
    #[serde(default, rename = "trait")]
    pub trait_: Option<String>,
    /// Survey-gated interpretations of the exact planetary combination. Empty
    /// before this viewer's survey wavefront; derived from the same multipliers
    /// production uses, so these are explanations rather than recommendations
    /// guessed independently by the client.
    #[serde(default)]
    pub opportunities: Vec<ColonyOpportunityView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColonyOpportunityView {
    pub role: String,
    pub title: String,
    pub tier: String,
    pub score: f64,
    pub body_id: Option<u32>,
    pub body_name: Option<String>,
    pub reason: String,
}

/// §node: the per-system view of an EXOTIC NODE. The bonus + awakened state are
/// public (everyone sees the landmark); `fed` and `region_radius` are OWNER-ONLY
/// (your own logistics + the region ring only you command), so a rival sees them as
/// `false`/`0.0`.
#[derive(Debug, Clone, Serialize)]
pub struct NodeStateView {
    /// Stable bonus slug — `relay_anchor` | `veil` | `deep_scan` (for the label).
    pub bonus: String,
    /// Human bonus title (e.g. "Relay Anchor").
    pub title: String,
    /// Has the node awakened (past the awakening time)? Before it, the system is a
    /// telegraphed dormant landmark; after, a live capturable prize.
    pub awakened: bool,
    /// OWNER-ONLY: is the node's upkeep currently met? An unfed node's bonus is
    /// SUSPENDED. Rivals always see false.
    pub fed: bool,
    /// OWNER-ONLY: the node's region radius (sim units) for the holder's map ring;
    /// rivals see 0.0.
    pub region_radius: f64,
}

/// The viewer's SYNDICATE (§syndicates Part 1) — their own roster, delivered in
/// the per-player view so the client can render membership + manage it. Only
/// ever the viewer's OWN syndicate (never a rival's private roster).
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateView {
    pub id: SyndicateId,
    pub name: String,
    pub founder: PlayerId,
    /// Whether the VIEWER is the founder (may invite / dissolve).
    pub is_founder: bool,
    /// The roster (id + display name), member-id ordered for determinism.
    pub members: Vec<SyndicateMember>,
    /// Outstanding invites the founder has sent (names), for the roster panel.
    pub invited: Vec<String>,
    /// §fitting: the syndicate's saved DOCTRINE FITS (named hull + loadout;
    /// any member curates). Owner-only like the rest of the view.
    #[serde(default)]
    pub fits: Vec<FitView>,
    /// §ladder B4: the christened name of the syndicate's Titan (None if
    /// unnamed / not fielded). Owner-only here; rivals meet it only in
    /// participant battle records.
    #[serde(default)]
    pub flagship_name: Option<String>,
    pub my_role: sim::SyndicateRole,
    /// §syndicate-projects (R6): the week's ladder and the bonuses.
    pub projects: SyndicateProjectsView,
}

/// §syndicate-projects (R6): one rung of the week's ladder.
#[derive(Debug, Clone, Serialize)]
pub struct RungView {
    pub goods: Commodity,
    pub goal: u32,
    pub points: u32,
}

/// One bonus track: the syndicate's level and the level that has reached you.
#[derive(Debug, Clone, Serialize)]
pub struct BonusView {
    pub bonus: sim::syndicate::SyndicateBonus,
    pub level: u8,
    pub mine: u8,
    /// The gain per level, as a fraction.
    pub per_level: f64,
    /// Points for the next level; None at the top.
    pub next_cost: Option<u32>,
    /// Seconds until your next level arrives, while you wait for one.
    pub next_in: Option<f64>,
}

/// §syndicate-projects (R6): the week's ladder, points and bonuses.
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateProjectsView {
    pub week: u32,
    /// Seconds until the week closes.
    pub week_ends_in: f64,
    /// The rung in progress (0-based); `rungs.len()` once the ladder is done.
    pub rung: usize,
    pub progress: u32,
    pub rungs: Vec<RungView>,
    pub points: u32,
    pub week_points: u32,
    pub bonuses: Vec<BonusView>,
    /// Goods each member gave this week, most first.
    pub given: Vec<(String, u32)>,
    /// Weekly project medals: gold, silver, bronze.
    pub medals: [u32; 3],
    /// Whether you may spend points.
    pub may_raise: bool,
}

/// §fitting: one saved doctrine fit on the wire (modules sorted, never empty —
/// an all-stock "fit" isn't storable; stock is the absence of a fit).
#[derive(Debug, Clone, Serialize)]
pub struct FitView {
    pub name: String,
    pub kind: ShipKind,
    pub modules: Vec<sim::ModuleKind>,
}

/// One member of a [`SyndicateView`] roster.
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateMember {
    pub id: PlayerId,
    pub name: String,
    pub role: sim::SyndicateRole,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationView {
    pub id: sim::OperationId,
    pub issuer: sim::OperationIssuer,
    pub scope: sim::OperationScope,
    pub kind: sim::OperationKind,
    /// ARRIVED state/progress, never operation truth ahead of its report.
    pub state: sim::OperationState,
    pub progress: u32,
    pub goal: u32,
    pub stage: u8,
    pub reported_at: f64,
    pub offered_at: f64,
    pub starts_at: f64,
    pub expires_at: f64,
    pub target_pos: Vec2,
    pub reward: sim::OperationReward,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub briefing: Option<sim::operation::OperationBriefing>,
    pub joined: bool,
    pub assigned_fleet: Option<EntityId>,
    pub winner: Option<PlayerId>,
    /// §contract-board: the day this contract was posted on the board.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_day: Option<u64>,
}

/// §signals: the corporation's signals near home. Owner-only. What a signal
/// holds stays off the wire until its report reaches command.
#[derive(Debug, Clone, Serialize)]
pub struct SignalsView {
    /// When the next signal is due; None while the open slots are full.
    pub next_at: Option<f64>,
    pub open_max: u32,
    /// The scripted first ten: how many reports are in, and what the next holds.
    pub first_ten: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_ten_next: Option<&'static str>,
    pub signals: Vec<SignalView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalView {
    pub id: EntityId,
    pub number: u32,
    pub kind: sim::signals::SignalKind,
    pub hazardous: bool,
    pub pos: Vec2,
    pub detected_at: f64,
    pub xp: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<SignalReportView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalReportView {
    pub at: f64,
    pub fleet: EntityId,
    pub captain: String,
    pub xp: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hurt_until: Option<f64>,
    pub finds: Vec<sim::signals::SignalFind>,
}

/// §contract-board: the corporation's board, as of the reports that have
/// reached command. Owner-only.
#[derive(Debug, Clone, Serialize)]
pub struct ContractBoardView {
    pub day: u64,
    /// When the next day's contracts are posted.
    pub next_at: f64,
    /// Contracts from this day's board paid out so far.
    pub done: u32,
    pub needed: u32,
    /// §standing-ladder (R3): the bonus pays standing and a dossier
    /// (fraction of a programme), not credits.
    pub bonus_standing: f64,
    pub bonus_dossier: f64,
    pub bonus_paid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiplomacyRelationView {
    pub other: PlayerId,
    pub name: String,
    pub state: sim::diplomacy::RelationState,
    pub war_activates_at: Option<f64>,
    pub separation_until: f64,
    pub reprisal_until: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TreatyProposalView {
    pub id: u64,
    pub from: PlayerId,
    pub name: String,
    pub treaty: sim::diplomacy::TreatyKind,
    pub expires_at: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiplomacyView {
    pub relations: Vec<DiplomacyRelationView>,
    pub incoming: Vec<TreatyProposalView>,
}

/// A pending invitation the VIEWER may accept (§syndicates Part 1).
#[derive(Debug, Clone, Serialize)]
pub struct SyndicateInviteView {
    pub id: SyndicateId,
    pub name: String,
}

/// §research R6: the viewer's CORPORATION research picture — owner-only (design
/// law 3: nothing leaks to rivals). The whole 108-programme tree with per-node
/// state + gate progress, the active programme with its live rate and ETA, the
/// queue, and the per-Academy contribution table (shown math, law 2).
#[derive(Debug, Clone, Serialize)]
pub struct ResearchView {
    pub blueprints: Vec<sim::ModuleKind>,
    /// The programme the clock is accruing into, if any.
    pub active: Option<ActiveResearchView>,
    /// The queue-ahead ids (front is the active once promoted).
    pub queue: Vec<String>,
    /// Total live throughput rate (Σ supplied Academy rates); 0 if stalled/starved.
    pub rate: f64,
    /// True while an available active programme has no staffed Academy.
    pub stalled: bool,
    /// The per-Academy contribution rows (shown factor chains).
    pub academies: Vec<AcademyRow>,
    /// §perf Part B: the DYNAMIC per-node slice only (state + gate) — the static
    /// catalog (names/blurbs/topology/cost) went once in Welcome.
    pub programmes: Vec<ProgrammeDynView>,
    /// §research-grant (R3): buying research-hours with credits.
    pub grant: ResearchGrantView,
    /// §designations (R2): where each researched capability points.
    pub designations: Vec<DesignationView>,
    /// §unlocks (v72): the highest level of each structure (by slug) this
    /// corporation may build now, `build::max_buildable_tier` on its research.
    /// 0: not unlocked yet. The client shows nothing above it.
    pub structure_tiers: std::collections::BTreeMap<String, u32>,
}

/// §designations: one capability's target, whether it is live (target still
/// held), and when it may next move (None: free now).
#[derive(Debug, Clone, Serialize)]
pub struct DesignationView {
    pub cap: sim::research::Cap,
    pub target: sim::research::DesignationTarget,
    pub live: bool,
    pub locked_until: Option<f64>,
}

/// §research-grant: one grant's price for the active programme (None with
/// nothing running), its research seconds, and how many are left today.
#[derive(Debug, Clone, Serialize)]
pub struct ResearchGrantView {
    pub price: Option<f64>,
    pub secs: f64,
    pub left: u32,
    pub per_day: u32,
}

/// §research R6: the active programme banner data.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveResearchView {
    pub id: String,
    pub name: String,
    /// Throughput-seconds accrued and the total cost.
    pub progress: f64,
    pub cost: f64,
    /// Seconds remaining at the current rate; None when the rate is 0.
    pub eta_secs: Option<f64>,
}

/// §research R6: one Academy's contribution row (the shown factor chain).
#[derive(Debug, Clone, Serialize)]
pub struct AcademyRow {
    pub system: String,
    pub body_id: u32,
    pub tier: u32,
    pub throughput: f64,
    pub staffing: f64,
    pub skill: f64,
    pub food: f64,
    /// §power: the planet's grid.
    pub power: f64,
    pub rate: f64,
    /// False → the lab can't cover its drip this tick (amber in the UI).
    pub supplied: bool,
}

/// §perf Part B: one programme's STATIC catalog entry — names, blurbs, board
/// topology, cost. The same constant table for everyone (the game's rulebook,
/// not anyone's progress), sent ONCE in Welcome instead of per-member at 10 Hz.
#[derive(Debug, Clone, Serialize)]
pub struct ProgrammeInfo {
    pub id: String,
    /// Field slug ("propulsion" …) — the board this node lives on.
    pub field: String,
    /// School slug ("line_haul" …); None for the shared Tier I/II rungs.
    pub school: Option<String>,
    pub tier: u8,
    pub name: String,
    pub blurb: String,
    /// Cost in throughput-seconds (the ETA denominator context).
    pub cost: f64,
    /// §rare-materials: the goods each Academy draws per rate-second while
    /// funding it (`sim::research::basket`).
    pub supplies: Vec<(sim::Commodity, f64)>,
}

/// §research R6 / §perf Part B: one programme node's DYNAMIC slice — the
/// viewer's state + gate progress. The client joins it onto the static
/// [`ProgrammeInfo`] catalog by id.
#[derive(Debug, Clone, Serialize)]
pub struct ProgrammeDynView {
    pub id: String,
    /// One of: "completed" | "active" | "queued" | "available" | "locked".
    pub state: String,
    /// For a LOCKED node whose tier carries a verb/metric gate: the progress bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateProgressView>,
    /// Arrived dossier work banked for this specific programme; never truth-side loot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovered_data: Option<f64>,
}

/// §research R6: a gate progress bar for a sealed tier.
#[derive(Debug, Clone, Serialize)]
pub struct GateProgressView {
    pub label: String,
    pub current: f64,
    pub threshold: f64,
}

/// §bodies: one PLANET OR MOON on the wire. Kind, size, environment and parent
/// are public astronomy. GEOLOGY, SPECIAL and DEPOSITS ride the survey ladder;
/// structures/population are OWNER-ONLY (empty/zero for rivals — the fog law
/// one level down, nothing new leaks).
#[derive(Debug, Clone, Serialize)]
pub struct BodyView {
    pub founding_camp: Option<sim::settlement::FoundingCamp>,
    /// Owner-only base accommodation; research multipliers are separate.
    pub population_capacity: Option<f64>,
    pub id: u32,
    pub name: String,
    /// Kind slug: rocky | terrestrial | ocean | ice | gas_giant | asteroid_belt.
    pub kind: String,
    pub parent: Option<u32>,
    /// §belts: an asteroid belt's type and picture. Belts are withheld until
    /// the viewer has charted (surveyed) the system.
    pub belt: Option<BeltView>,
    /// §real-coordinates: where it goes round (System View units, 1 =
    /// `sim::body::SYSTEM_UNIT_SU`): about its star, or its planet for a moon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orbit: Option<OrbitView>,
    pub habitable: bool,
    pub size: String,
    pub environment: String,
    /// Survey-gated mineral grade slug.
    pub geology: Option<String>,
    /// Survey-gated rare body feature and its exact effect copy.
    pub special: Option<String>,
    pub special_effect: Option<String>,
    /// Public natural multipliers: useful before committing a colony ship.
    pub habitat_capacity_mult: f64,
    pub population_growth_mult: f64,
    pub provisions_mult: f64,
    pub construction_time_mult: f64,
    /// Owner-only estimate from the arrived administration/research report.
    pub structure_build_time_mult: Option<f64>,
    /// Survey-gated mineral multiplier (the grade before resource-specific specials).
    pub mineral_extraction_mult: Option<f64>,
    /// Owner-only live per-body pools; None cannot leak rival population tiers.
    pub resource_slots: Option<u32>,
    pub industrial_slots: Option<u32>,
    pub infrastructure_slots: Option<u32>,
    /// §research-wiring Xenoacclimation: a Marginal world that counts as
    /// habitable for its owner (owner-only).
    pub acclimated: bool,
    /// Survey-gated because Low Gravity is itself surveyed.
    pub ship_build_time_mult: Option<f64>,
    /// This body's deposits — survey-gated like the system list.
    pub deposits: Option<Vec<DepositView>>,
    /// §pockets: the hidden pockets of this belt that the VIEWER's corporation
    /// has found (its report reached command). Never another corporation's.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pockets: Vec<DepositView>,
    /// §pockets: prospecting passes the viewer has had reported here.
    #[serde(skip_serializing_if = "is_zero")]
    pub prospected: u32,
    /// OWNER-ONLY: structures on this body (slug → tier); empty for rivals.
    pub structures: BTreeMap<String, u32>,
    /// OWNER-ONLY: population on this body (millions); 0 for rivals.
    pub population: f64,
    /// OWNER-ONLY civilian immigration policy; rivals receive `closed`.
    pub migration_policy: sim::MigrationPolicy,
    /// OWNER-ONLY people already aboard liners bound here; 0 for rivals.
    pub inbound_migrants: u32,
    /// §migration-status: OWNER-ONLY housing and liner status; None for rivals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration: Option<MigrationView>,
    /// §power: OWNER-ONLY, this planet's grid as reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power: Option<PowerView>,
}

/// §migration-status (Nov 1 2026): one own world's immigration, from the
/// Hub's allocation pass over the arrived report of its system.
#[derive(Debug, Clone, Serialize)]
pub struct MigrationView {
    /// Housing with research (millions), as the allocation reads it.
    pub housing: f64,
    /// Liners bound here, and the sim time the first lands.
    pub liners: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lands_at: Option<f64>,
    /// Why no new liner comes here; None when it can take one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<sim::migration::MigrationHold>,
    /// The corporation's next liner comes here, allocated at this sim time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_at: Option<f64>,
}

/// §power: one planet's grid — MW drawn against MW available, the factor on
/// every powered structure here, and its plant's burn.
#[derive(Debug, Clone, Serialize)]
pub struct PowerView {
    pub demand: f64,
    pub supply: f64,
    pub power: f64,
    /// Units of `fuel` per second.
    pub burn: f64,
    /// §fuel-overhaul: what the plant burns — none on a Solar Array.
    pub fuel: Option<Commodity>,
    pub no_fuel: bool,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// §belts: a charted belt's type (stony | metallic | icy | shattered) and its
/// place in the System View picture (orbit radius and band width, normalised).
#[derive(Debug, Clone, Serialize)]
pub struct BeltView {
    pub kind: String,
    pub radius: f64,
    pub width: f64,
}

/// §real-coordinates: a world's orbit — radius (System View units), angle at
/// sim time 0 (radians) and seconds per turn (0 for a belt).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct OrbitView {
    pub radius: f64,
    pub angle: f64,
    pub period: f64,
}

/// §economy Part 6: a colony's workforce numbers — owner-only.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorkforceView {
    /// Workforce units the population fields (`floor(pop / 0.8M)`).
    pub units: u32,
    /// Crews posted across all assignments (may exceed `units` — every line
    /// then dilutes by the same share).
    pub posted: u32,
}

/// §economy Part 6: one production line, with the RESOLVED factor chain the
/// client shows verbatim (no hidden math). Owner-only.
#[derive(Debug, Clone, Serialize)]
pub struct AssignmentView {
    /// §bodies: the body whose line this is.
    pub body_id: u32,
    /// Structure slug (matches build keys / icons).
    pub structure: String,
    pub title: String,
    pub tier: u32,
    pub workers: u32,
    pub refining_ore: Option<Commodity>,
    /// Specialists posted to this line (kind → n).
    pub specialists: BTreeMap<sim::SpecialistKind, u32>,
    /// Why the line is stopped (`no_food` / `no_inputs` / `storage_full`), if it is.
    pub suspended: Option<String>,
    /// The factor chain, resolved this tick.
    pub throughput: f64,
    pub staffing: f64,
    pub skill: f64,
    pub food: f64,
    /// §power: the planet's grid (1 for structures that draw none).
    pub power: f64,
    /// Planetary geology/special factor. Age/crew thresholds never enter it.
    pub site: f64,
    pub recovery: f64,
    /// §research-wiring: the input basket's multiple (Autoforges, crops).
    pub input_mult: f64,
    /// Net output lines at those factors (commodity, units/s) — extraction
    /// lists each deposit's commodity; a converter lists its output.
    pub outputs: Vec<(Commodity, f64)>,
}

/// §economy: a built CONVERTER's live status, for the system-view idle banner:
/// `running`, or WHY it produces nothing — `needs_crew` (built but no crew
/// posted), or a staffed line's latched outage (`no_inputs` / `no_food` /
/// `storage_full`). Owner-only; mirrors the tick's converter gate so the banner
/// never contradicts what the sim actually did.
#[derive(Debug, Clone, Serialize)]
pub struct ConverterStatusView {
    pub body_id: u32,
    pub structure: String,
    pub title: String,
    pub tier: u32,
    /// running | needs_crew | no_inputs | no_food | storage_full
    pub status: String,
    /// Owner's arrived rated capacity, even before workers are assigned.
    pub rated_output: f64,
    pub site: f64,
    pub recovery: f64,
    pub input_mult: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefiningSiteView {
    pub body_id: u32,
    pub tier: u32,
    pub built: bool,
    /// Output per base input basket, including geology and recovery research.
    pub recovery: f64,
    /// Fully staffed work multiplier, before the chosen ore recipe's base rate.
    pub work_rate: f64,
}

/// A convoy's cargo manifest, as revealed to a player whose sensors are within
/// range (Tier 2). Absent from the ghost when out of sensor coverage.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CargoView {
    pub commodity: Commodity,
    pub units: u32,
}

/// One of the player's in-flight order LIFECYCLES (§order-lifecycle), OWNER-ONLY.
/// The client derives its estimated phase from `sim_time`: SIGNAL OUTBOUND until
/// `arrives_at`, then PRESUMED DELIVERED while awaiting arrived evidence. Passing
/// `response_at` never confirms anything; it only makes the estimate overdue. Both
/// stamps are solved once from the served ghost at issue — never authoritative
/// fleet truth — so the panel and outbound comet share one command-center clock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOrderView {
    pub id: u64,
    pub fleet_id: EntityId,
    pub issued_at: f64,
    pub arrives_at: f64,
    pub response_at: f64,
    pub kind: OrderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dest: Option<Vec2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emplacement: Option<sim::emplace::EmplacementKind>,
    /// Exact owner-issued configuration carried by this signal. This is
    /// command knowledge, not fleet truth, and lets controls stay visibly
    /// pending until a served report confirms the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<sim::world::PendingConfigurationView>,
    /// Owner-only intended route from the served sighting to a fixed destination.
    /// Positions only: this line never claims the fleet has advanced along it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intent_path: Vec<Vec2>,
    /// Terminal only after the jump-loss evidence reaches this owner. Before
    /// that same wavefront the sim may already know the truth, but the field is
    /// deliberately false and the row continues its ordinary estimate.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lost: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_relay: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_break: Option<Vec2>,
}

/// An ongoing BATTLE as any observer perceives it (§battles-take-time), STRICTLY
/// light-gated: it appears only once the light of its start has reached the
/// viewer's command center. Weapons fire is loud — all participants (even dark
/// fleets) are revealed at the site by that same old light. `age` is how stale
/// the sighting is ("battle raging — as of N ago").
#[derive(Debug, Clone, Serialize)]
pub struct BattleView {
    /// The engagement's stable id — ONE battle entity, ONE map icon. Merging
    /// reinforcements join the same entity, so the id (and the icon) is stable.
    pub id: EntityId,
    pub pos: Vec2,
    /// Straight warp-light delay of the battle sighting (seconds).
    pub age: f64,
    /// Sim-time the battle began (for the panel's observed-elapsed readout).
    pub started_at: f64,
    /// True if the viewer is one of the two sides (they read their own running
    /// losses by their own light via the delayed reports).
    pub own: bool,
    /// The battle's participant fleet ids — exactly the set revealed to any
    /// observer of the battle by the existing weapons-fire site-reveal (their
    /// ghosts are already sent). The client uses these to SUPPRESS each
    /// participant's own map marker (the icon carries the state) and to build
    /// the battle panel. No new information beyond the ghosts already revealed.
    pub participants: Vec<EntityId>,
}

// --- §battle-records Part A2: the light-gated, fidelity-tiered replay ---------
//
// Every observable battle carries a per-viewer replay: the ARRIVED-round prefix
// (round `i` unlocks when `round.tick·DT + |pos−cc|/c ≤ now`, the same light
// gate as the battle-news event) at the viewer's fidelity. A PARTICIPANT sees
// everything (their own posture only, never the opponent's); a THIRD PARTY who
// currently covers the site sees a BUCKETED spine (CountClass only, no dealt, no
// notes beyond joins/mutual-disengage); anyone else gets no record at all (just
// the existing news + wreck marker). Nothing beyond the light frontier ships.

/// A viewer's fidelity on a battle record.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleFidelity {
    /// A participant corp: exact counts, kills, damage dealt, and every beat.
    Participant,
    /// A third party covering the site: the bucketed spine only.
    Bucket,
    /// §research-wiring Battle Archives: a third party's bucketed spine with
    /// each round's losses, bucketed.
    Archive,
}

/// One `(kind, strength)` entry of a recorded side. `exact` is present ONLY at
/// participant fidelity; `class` (the [`CountClass`] bucket) is ALWAYS present —
/// so a third party learns "4–7 Corvettes", never the true count (leak-safe).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RecordCount {
    pub kind: ShipKind,
    pub exact: Option<u32>,
    pub class: CountClass,
}

/// One side of a recorded battle as a viewer sees it.
#[derive(Debug, Clone, Serialize)]
pub struct SideRecordView {
    pub corp: PlayerId,
    /// The corp's engagement doctrine — OWNER-ONLY: `Some` iff this is the
    /// viewer's own side. A rival never learns your posture from the replay.
    pub posture: Option<EngagementPolicy>,
    pub platform_tiers: u32,
    pub initial: Vec<RecordCount>,
    /// §modules B5: the side's opening FITTED stacks (kind + modules + count) —
    /// PARTICIPANT ONLY (empty at bucket/none fidelity, a fog-safe leak guard).
    /// The client labels the side and types its salvos by dominant weapon family.
    #[serde(default)]
    pub loadouts: Vec<LoadoutStack>,
    /// §ladder B4: the christened name of this side's TITAN — PARTICIPANT ONLY
    /// and only when the side fielded one. How a rival ever meets the name.
    #[serde(default)]
    pub flagship_name: Option<String>,
}

/// A recorded beat, viewer-filtered. `kind` is the snake_case note tag; `side`
/// and `comp` accompany the beats that carry them (`joined`).
#[derive(Debug, Clone, Serialize)]
pub struct RoundNoteView {
    pub kind: String,
    pub side: Option<u8>,
    pub comp: Option<Vec<RecordCount>>,
}

/// One recorded round as a viewer sees it. Indexing is by side (0 = attackers,
/// 1 = defenders): `counts[s]` survivors of `s`, `kills[s]` losses of `s`.
#[derive(Debug, Clone, Serialize)]
pub struct RoundRecordView {
    pub tick: u64,
    pub counts: [Vec<RecordCount>; 2],
    pub kills: [Vec<RecordCount>; 2],
    /// Damage each side dealt — PARTICIPANT ONLY (`None` at bucket fidelity).
    pub dealt: Option<[f64; 2]>,
    pub notes: Vec<RoundNoteView>,
    /// §tactical T3: the round's TRUTH KEYFRAME (real positions, torpedo
    /// salvos, exact deaths) — PARTICIPANT ONLY, absent on old records (the
    /// client falls back to choreographed rendering).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<sim::combat::Keyframe>,
}

/// A battle's replay as one viewer perceives it — light-gated + fidelity-tiered.
#[derive(Debug, Clone, Serialize)]
pub struct BattleRecordView {
    pub id: EntityId,
    pub pos: Vec2,
    pub system: Option<EntityId>,
    /// Sim-time the battle began (`started_tick × DT`) — the "as of N ago" line.
    pub started_at: f64,
    pub raid: bool,
    pub fidelity: BattleFidelity,
    /// Which side (0/1) is the viewer's own, if any — drives the owner-only
    /// posture display and the "you" framing.
    pub own_side: Option<u8>,
    pub sides: [SideRecordView; 2],
    /// The ARRIVED round prefix at the viewer's fidelity (light-gated per round).
    pub rounds: Vec<RoundRecordView>,
    /// Newest arrived round tick — the client's light frontier (rounds beyond it
    /// draw as the hatched "beyond your light cone" zone).
    pub light_frontier_tick: u64,
    /// The outcome, once the FINAL round's light has arrived — `None` while the
    /// battle is still (as far as this viewer's light shows) running.
    pub outcome: Option<RaidOutcome>,
}

// --- §ground G2: LANDING RECORDS ---------------------------------------------
// A landing is a fight that takes time now, so it leaves a replayable account.
// Everything here follows the battle record's fog discipline exactly: opening
// light gates the record's existence, each round arrives on its own light, and
// a third party covering the site sees the SHAPE of the fight without its
// numbers. It rides the same reliable delta lane for the same reason — the View
// is last-write-wins, and re-shipping whole records per tick was a confirmed
// source of stutter.

/// How much of a landing a viewer can resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundFidelity {
    /// Attacker or defender: exact troop counts, casualties, every beat.
    Participant,
    /// A third party covering the site from orbit: they can see a landing is
    /// being fought and roughly how it is going — never the true headcounts.
    Bucket,
}

/// One recorded round as a viewer sees it. `marines`/`defenders` carry exact
/// numbers ONLY at participant fidelity; the `*_frac` fields (0..1 of each
/// side's opening strength) are always present, so a bucket observer can watch
/// the shape of the fight without ever learning its arithmetic.
#[derive(Debug, Clone, Serialize)]
pub struct GroundRoundView {
    pub tick: u64,
    pub marines: Option<u32>,
    pub defenders: Option<u32>,
    pub marines_frac: f64,
    pub defenders_frac: f64,
    /// 0..1 of the garrison pinned by bombardment over this round. Visible to
    /// everyone who can see the fight at all — the guns are not subtle.
    pub suppression: f64,
    pub marine_losses: Option<u32>,
    pub defender_losses: Option<u32>,
    /// Derived beats: guns_lifted / guns_resumed / garrison_starved / tipped.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub notes: Vec<String>,
}

/// A landing's per-viewer header — everything except the rounds. Sent once per
/// record per connection; the rounds then stream as their light arrives.
#[derive(Debug, Clone, Serialize)]
pub struct GroundRecordHeader {
    pub system: EntityId,
    pub pos: Vec2,
    pub started_at: f64,
    pub fidelity: GroundFidelity,
    pub attacker: PlayerId,
    pub defender: PlayerId,
    /// True when the viewer is the one who put the men down (drives the "you"
    /// framing, and nothing else — both sides read the same fight).
    pub attacking: bool,
    /// Opening strengths — participant only.
    pub marines_landed: Option<u32>,
    pub defenders_initial: Option<u32>,
    /// Garrison tiers that stood here. Public in the sense that anyone watching
    /// a landing can see how dug-in the ground was.
    pub garrison_tiers: u32,
    pub suppression_at_drop: f64,
    /// §veil: marines lost in the descent — participant only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descent_losses: Option<u32>,
}

/// An incremental update for one landing record.
#[derive(Debug, Clone, Serialize)]
pub struct GroundRecordUpdate {
    pub id: EntityId,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub header: Option<GroundRecordHeader>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub new_rounds: Vec<GroundRoundView>,
    pub light_frontier_tick: u64,
    /// `taken` / `repulsed`, once the FINAL round's light has arrived.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outcome: Option<String>,
}

/// §perf Part A: a record's per-viewer HEADER — everything except the rounds.
/// Sent once per record per connection (and again only if a christened flagship
/// name changes); the rounds then stream incrementally as their light arrives.
#[derive(Debug, Clone, Serialize)]
pub struct BattleRecordHeader {
    pub pos: Vec2,
    pub system: Option<EntityId>,
    pub started_at: f64,
    pub raid: bool,
    pub fidelity: BattleFidelity,
    pub own_side: Option<u8>,
    pub sides: [SideRecordView; 2],
}

/// §perf Part A: one record's INCREMENT for one connection — only what that
/// connection hasn't received yet. `header` is present when the record is new to
/// the connection (or its header changed); `new_rounds` appends to what the
/// client already holds; `outcome` is sent exactly once, when its light arrives.
/// Everything inside is filtered per-viewer exactly as [`BattleRecordView`] was —
/// the cursor changes WHEN data ships, never WHAT a viewer may see.
#[derive(Debug, Clone, Serialize)]
pub struct BattleRecordUpdate {
    pub id: EntityId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<BattleRecordHeader>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub new_rounds: Vec<RoundRecordView>,
    /// The viewer's current light frontier for this record (always fresh).
    pub light_frontier_tick: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<RaidOutcome>,
}

/// A home anchor as a player perceives it. `pos` is static geography; `owner`
/// is light-gated by the view filter — it is `None` to a player until the light
/// of the claim event has reached their command center (a rival's presence must
/// not be learned faster than light).
#[derive(Debug, Clone, Serialize)]
pub struct AnchorView {
    pub pos: Vec2,
    pub owner: Option<PlayerId>,
}

/// One (kind, count) entry of a fleet's exact composition — revealed only to
/// the owner, or to a rival whose sensors cover the fleet (Tier 2). Ordered by
/// kind for a stable wire form.
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
pub struct CompCount {
    pub kind: ShipKind,
    pub count: u32,
}

/// §reactors-drives: `count` engine-bearing hulls on `reactor` and `drive`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveView {
    pub reactor: sim::fuel::ReactorGrade,
    pub drive: sim::fuel::Drive,
    pub count: u32,
}

/// §modules Part B: one FITTED stack of a fleet — `n` ships of `kind` all carrying
/// `modules` (a canonical, sorted loadout; never empty — unfitted ships aren't
/// stacks). Revealed under the same rule as [`CompCount`].
#[derive(Debug, Clone, Serialize)]
pub struct LoadoutStack {
    pub kind: ShipKind,
    pub modules: Vec<sim::ModuleKind>,
    pub n: u32,
}

/// One point of an own fleet's direct planned flight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPointView {
    pub pos: Vec2,
}

/// §emplacements: the timed job a hull is holding station to finish. One shape
/// for both directions of the same mechanic — raising a structure and wrecking
/// one — so the panel needs no per-order special case, and a future repair or
/// refit job slots in without another wire field.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Building,
    Demolishing,
}

/// §emplacements: a hull's job and how far along it is (0..1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct JobView {
    pub kind: JobKind,
    pub progress: f64,
}

/// Observable jump-drive state at the retarded sighting. `remaining` is the
/// spool time that remained WHEN THIS REPORT LEFT the fleet; it is sent only
/// with a fleet already visible to this viewer, so it reveals neither a hidden
/// hull nor the jump destination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct JumpSpoolView {
    pub remaining: f64,
    pub waiting_for_fuel: bool,
}

/// A rival jump departure whose light has reached this viewer. This is a
/// destination-free historical fact: the player saw the fleet leave `pos`, but
/// learns nothing here about where it went.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JumpDepartureView {
    pub fleet: EntityId,
    /// Identity already visible on the detected pre-jump ghost. Retaining it on
    /// the historical event reveals no destination or post-jump information.
    pub owner: PlayerId,
    pub owner_name: String,
    pub kind: ShipKind,
    pub pos: Vec2,
    pub departed_at: f64,
    pub learned_at: f64,
}

/// Owner-only presentation of a jump whose departure report has arrived before
/// any report emitted at its destination. The destination is player-authored,
/// so its position is known; the marker remains explicitly provisional until
/// destination light catches up.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct JumpPresumptionView {
    /// The steady-state information delay at the new location.
    pub information_delay: f64,
    /// Seconds until the first destination-origin report is expected to arrive.
    pub report_in: f64,
    /// Direction of the instantaneous relocation, for the map arrow.
    pub heading: Vec2,
}

/// One hop of an order signal's path. `frac` is the hop's
/// arrival as a fraction of the signal's whole travel window, so the client
/// animates the comet with no speed constants of its own and the last hop is
/// always exactly 1.0 (landing with `arrive_time`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalHopView {
    pub pos: Vec2,
    pub frac: f64,
}

/// An interdiction field in open space, as its arrived light reports it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterdictionFieldView {
    pub id: EntityId,
    pub pos: Vec2,
    pub radius: f64,
    /// Raised by one of your fleets.
    pub own: bool,
    pub fleet: EntityId,
    /// When it lapses at the latest (it collapses sooner if its destroyer
    /// leaves or is lost).
    pub expires_at: f64,
}

/// §emplacements: one structure standing in open space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmplacementView {
    pub id: EntityId,
    pub kind: sim::EmplacementKind,
    pub pos: Vec2,
    /// Yours, or a rival's. A rival's structures appear only inside your sensor
    /// coverage (they are stationary, so once seen there is nothing further to
    /// learn about where they are) — and are the only ones you may tear down.
    #[serde(default)]
    pub own: bool,
    /// The sensor radius the map may draw without mirroring sim constants.
    pub sensor_range: f64,
}

/// A FLEET as a player perceives it: a delayed "ghost" — the position the light
/// now arriving at their command center shows, plus how stale that is and how
/// much the object could have moved since (§6). This is the ONLY fleet
/// information a player receives; never the true present state, never another
/// player's view. Deliberately omits the fleet's standing order (internal truth).
///
/// The two-tier INTEL LADDER (GDD §13.1): `count_class` (an estimated-size
/// bucket) is present on every visible fleet — a far observer of a broadcasting
/// hammer knows roughly HOW BIG it is; `composition` (exact kinds + counts) is
/// revealed ONLY within sensor coverage (or for your own fleets), exactly like
/// cargo. You know a fleet is inbound and roughly its size long before you learn
/// what is IN it.
#[derive(Debug, Clone, Serialize)]
pub struct CaptainView {
    pub id: u32,
    pub name: String,
    pub portrait: sim::CaptainPortrait,
    pub level: u8,
    pub title: sim::CaptainTitle,
    pub portrait_age: sim::CaptainPortraitAge,
    pub command_capacity: u32,
    pub xp: u32,
    pub next_level_xp: u32,
    pub unspent: u8,
    pub attributes: sim::CaptainAttributes,
}

/// One entry in the owner's corporation-wide officer ledger. Assigned
/// progression is copied from the same served fleet sample as the map ghost;
/// `report = None` means command has not yet received an officer report.
#[derive(Debug, Clone, Serialize)]
pub struct CaptainRosterView {
    pub id: u32,
    pub name: String,
    pub portrait: sim::CaptainPortrait,
    pub assigned_fleet: Option<EntityId>,
    pub stationed_system: Option<EntityId>,
    pub recovering_until: Option<f64>,
    /// §signals: hurt on a Hazardous signal; sits signals out until then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hurt_until: Option<f64>,
    /// Withheld until the casualty report reaches command. `killed` is
    /// permanent; the other outcomes retain this officer through recovery.
    pub loss_fate: Option<sim::CaptainLossFate>,
    pub report: Option<CaptainView>,
}

/// §research-wiring Wake Analysis: a contact's tank in quarters (0–1) and the
/// bearing it came from (radians, to the nearest sixteenth of a turn; None
/// while it holds still).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WakeView {
    pub fuel: f64,
    pub from: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GhostView {
    pub id: EntityId,
    pub owner: PlayerId,
    /// The FLAGSHIP kind — what the fleet is drawn and named for (precedence
    /// colony > convoy > corvette > raider > scout). A fleet-of-one is that ship.
    pub kind: ShipKind,
    /// Where the object was when the arriving light left it (retarded position).
    pub pos: Vec2,
    /// Factual velocity at that retarded moment (for heading and ship facts).
    pub vel: Vec2,
    /// Light delay in seconds — how stale this sighting is ("seen Xs ago").
    ///
    /// This is the certainty story for every rival and every own fleet.
    pub age: f64,
    /// True if this is one of the viewing player's own ships.
    pub own: bool,
    /// This served frame crossed a discontinuous jump. It drives owner order
    /// confirmation and the rival's arrival warning. The ghost has already
    /// passed visibility/detection gating, so this reveals no hidden contact and
    /// arrives on exactly the same delayed wavefront as the destination picture.
    pub jumped: bool,
    /// Owner-only, sampled with the fleet: personnel progression obeys the same
    /// information delay as the hull instead of leaking through a fresh roster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captain: Option<CaptainView>,
    /// §dock: the BERTH this sighting was taken at — `"hub"` for the
    /// Market Hub, otherwise the system's id — or null if the fleet was under
    /// way (or loitering somewhere it does not control).
    ///
    /// Deliberately NOT gated behind the composition reveal, unlike `cargo` or
    /// `damage`. Being parked at a station is plain from the position and
    /// velocity this ghost already carries; withholding it would conceal nothing
    /// and would make fleets vanish from the map inconsistently, since the
    /// client keys its declutter on this field. It is light-delayed like the
    /// rest of the sighting: a fleet you see berthed may have sailed since.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docked: Option<String>,
    /// §course-change: what the DRIVES were doing — cruising, spinning up, or
    /// shutting down. THE one drive field on the wire: the regime is derived
    /// from it client-side rather than sent alongside, because two fields that
    /// must agree eventually do not. Spooling looks exactly like thrusters from
    /// outside, so without this the panel cannot say why a ship is crawling.
    /// Light-delayed with the rest of the sighting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive: Option<sim::ship::DriveState>,
    /// Observable jump spool telemetry from this same retarded frame. Kept
    /// separate from `drive`: `DriveState` governs realspace motion, while a
    /// jump spool is a stationary, world-managed fleet order. A rival may see
    /// the charge but never the owner-only reason for a stalled spool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_spool: Option<JumpSpoolView>,
    /// Owner-only provisional jump bookmark. Present only when departure light
    /// has reached command before destination light; absent when the new delay
    /// is equal or shorter, because there is no information gap to depict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_presumed: Option<JumpPresumptionView>,
    /// Speed at the retarded moment (sim units / s) — the magnitude of `vel`,
    /// sent alongside it so the panel does not have to recompute what the server
    /// already knows.
    #[serde(default)]
    pub speed: f64,
    /// Owner-only bunker telemetry from this same retarded sighting.
    /// §fuel-overhaul: `fuel` is the fusion tank in Deuterium of energy
    /// (including `helium3` units, six each); `fuel_capacity` its space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_capacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helium3: Option<f64>,
    /// §shipbuilding-overhaul: the Helium Cores' tank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helium3_capacity: Option<f64>,
    /// §shipbuilding-overhaul: owner-only, the hulls aboard by warp-core grade
    /// (one tank per grade's fuel: `uranium`, `fuel` = Deuterium, `helium3`,
    /// `antimatter`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drives: Option<Vec<DriveView>>,
    /// §fuel-overhaul: the fission store (Uranium, Slow warp).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uranium: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uranium_capacity: Option<f64>,
    /// §fuel-overhaul: the antimatter bottle (jumps); absent capacity 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antimatter: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antimatter_capacity: Option<f64>,
    /// §shipbuilding-overhaul Phase 5: owner-only, the magazines — Torpedoes
    /// for the racks, Slugs for the drivers, Beam Crystals in the beam guns —
    /// with what the guns aboard hold; absent when nothing aboard takes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torpedoes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torpedoes_capacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slugs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slugs_capacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crystals: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crystals_capacity: Option<f64>,
    /// §syndicate-projects (R6): owner-only, the Freight bonus on player
    /// freighter holds (absent at 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_mult: Option<f64>,
    /// §holds: owner-only, Prize Holds' bonus on Combat Freighter holds
    /// (absent at 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prize_mult: Option<f64>,
    /// Owner-only: this sighting found the fleet held for lack of carried fuel.
    #[serde(default)]
    pub stalled: bool,
    /// Owner-only immediate administrative fact: AAA has accepted this fleet's
    /// callout and its physical tender is outbound.
    #[serde(default)]
    pub rescue_inbound: bool,
    /// Owner-only ledger carried by the same arrived sample as fuel/cargo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_transfer: Option<sim::ship::FuelTransfer>,
    /// Public role identity for an Authority Astral Assistance tender.
    #[serde(default)]
    pub rescue_service: bool,
    /// §merchant: public role identity for the traveling merchant's ship.
    #[serde(default)]
    pub merchant: bool,
    /// §traders: the free trader flying the merchant's ship (`teo`, `ines`,
    /// `mags`, `rafi`), so the map draws that trader's own ship.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trader: Option<String>,
    /// An observable convoy route (waypoints), light-delayed like its position.
    /// A silent rival convoy has no ghost at all outside detection coverage.
    pub route: Option<Vec<Vec2>>,
    /// §course-plan: OWN fleets only — the remaining legs of the flight the sim
    /// is actually flying through realspace. Rivals get `None`: your flight plan is
    /// yours. (Current-state like `docked`, with the same staleness caveat.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<PathPointView>>,
    /// Owner-only, light-delayed identity of the fleet this Interceptor is
    /// guarding. It is sampled with `path`, so assignment compliance never
    /// appears before the served fleet picture reaches it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_target: Option<EntityId>,
    /// Owner-only standing post, captured at this ghost's emission time, also
    /// present during sorties. Never projected from issued/pending commands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defend_system: Option<sim::ship::SystemDefense>,
    /// §emplacements: OWN fleets only — the timed job this hull is holding
    /// station to finish (raising a structure, or tearing a rival's down) and
    /// how far along it is. Absent when it is doing neither. Rivals never get
    /// it: what a parked hull is working on is not readable from outside.
    ///
    /// Drives BOTH the panel's progress bar and its order lockout — without the
    /// latter a stationary worker looks idle and a second order dies in silence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<JobView>,
    /// §economy Part 4: specialist PASSENGERS aboard — part of the manifest,
    /// included under exactly the cargo rule below (empty = none visible).
    pub passengers: BTreeMap<sim::SpecialistKind, u32>,
    /// Compatibility alias for the first cargo stack. New clients consume
    /// `cargo_manifest`; retained so a rolling client does not lose all cargo.
    pub cargo: Option<CargoView>,
    /// The convoy's complete mixed manifest — present ONLY when this convoy is
    /// within the viewing player's sensor coverage (Tier 2). Empty out of range,
    /// empty-hold, or for a hull without cargo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cargo_manifest: Vec<CargoView>,
    /// The estimated-size BUCKET (`1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+`). Always
    /// present on any visible fleet — the honest, un-invertible size estimate a
    /// fog observer gets even for a fleet far outside sensor coverage.
    pub count_class: CountClass,
    /// The EXACT composition (kinds + counts). Present ONLY for the viewer's own
    /// fleets, or a rival fleet inside sensor coverage (Tier 2). `None` otherwise
    /// — you have the size bucket but not the makeup. Never leaks the true count.
    pub composition: Option<Vec<CompCount>>,
    /// §roster: how BEATEN UP the fleet is — 0.0 pristine … 1.0 every hull spent.
    /// An AGGREGATE FRACTION, never the per-hull roster: shipping individual
    /// hulls would hand a distant observer the exact count that `count_class`
    /// exists to withhold. Present under exactly the `composition` rule, so a
    /// wounded fleet reads as wounded only from inside sensor coverage — which
    /// makes finding one a real intel prize.
    #[serde(default)]
    pub damage: Option<f64>,
    /// §modules Part B: the FITTED stacks (kind + modules + count). Present under
    /// exactly the `composition` rule — seeing the makeup reveals the fits. Only
    /// non-default stacks; the unfitted remainder = composition − Σ these.
    #[serde(default)]
    pub loadouts: Option<Vec<LoadoutStack>>,
    /// The dark fleet's DETECTION SIGNATURE (§Part 4) at the retarded moment — how
    /// LOUD it is (1.0 = a lone raider at full speed). Present only for DARK
    /// fleets; drives the client's flare/plume treatment. `None` for broadcasters.
    pub signature: Option<f64>,
    /// The fleet's ENGAGEMENT POSTURE (§offensive-orders Part 2) — OWNER-ONLY, so
    /// `Some(..)` for your own fleets and `None` for every rival (a standing
    /// per-fleet policy is private, like the corp doctrine; never leaks).
    pub posture: Option<EngagementPosture>,
    #[serde(default)]
    pub mission_profile: Option<sim::doctrine::MissionProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub industry: Option<sim::industry::FleetIndustry>,
    /// §belts: OWNER-ONLY — a Mining Belt Station's anchor at a belt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<sim::industry::Anchor>,
    #[serde(default)]
    pub pirate_faction: Option<sim::pirate::PirateFaction>,
    /// §explore Part 2: SURVEY DWELL progress (0..1) — OWNER-ONLY (your own
    /// fleet's live order state, like `posture`; a rival never sees it — they
    /// see only the louder signature under the normal detection rules). `None`
    /// when not dwelling.
    #[serde(default)]
    pub survey_progress: Option<f64>,
    /// OWN fleets only: when its interdiction field generator can raise
    /// another field (absent: ready, or it carries none), as of this picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interdiction_ready: Option<f64>,
    /// Arrived owner-only site assignment; never synthesized from a pending order.
    #[serde(default)]
    pub expedition: Option<sim::sites::ExpeditionAssignment>,
    /// True if this fleet's owner is a SYNDICATE ally as the viewer KNOWS it
    /// (§syndicates Part 1) — light-delayed membership (`World::known_ally`), so a
    /// fresh join/leave isn't seen early. Drives the friendly ally tint/pip.
    #[serde(default)]
    pub ally: bool,
    /// §syndicates Part 3: OWNER-ONLY. When this is one of YOUR fleets stationed as
    /// an ally GARRISON, the host system it defends; `None` otherwise (and always
    /// for rivals — a private status). `garrison_fed` = the host is covering its
    /// Provisions upkeep (else its defense contribution is suspended).
    #[serde(default)]
    pub garrison_host: Option<EntityId>,
    #[serde(default)]
    pub garrison_fed: bool,
    /// §upkeep: OWNER-ONLY. Is this fleet's standing Provisions upkeep being met?
    /// `false` = IMMOBILIZED: it accepts no new movement or offensive order until
    /// food reaches it (it keeps its guns and its current course, and loses
    /// nothing). Private quartermastery — a rival never learns you can't feed
    /// your navy, which would be a gift. serde default `true`.
    #[serde(default)]
    pub supplied: bool,
    /// §pirates: this fleet belongs to the neutral PIRATE faction (a raider pack) —
    /// drives the distinct hostile-neutral tint. Hostile to everyone.
    #[serde(default)]
    pub pirate: bool,
    /// §TCA: this fleet is a Terran Charter Authority FREIGHTER — the scheduled
    /// common carrier. Drives its own neutral tint, distinct from a corp convoy.
    #[serde(default)]
    pub tca: bool,
    /// This Authority hull is a civilian migrant liner rather than commodity
    /// freight. Identity is sampled with the track and remains light-delayed.
    #[serde(default)]
    pub migrant: bool,
    /// §TCA: the freighter's MANIFEST as this viewer may read it — their own lots
    /// always, everyone else's only from inside sensor range. Empty for any other
    /// fleet, and empty for a freighter a distant rival is merely watching.
    #[serde(default)]
    pub manifest: Vec<ManifestEntryView>,
    /// Whether this ghost is close enough for the viewer to read its manifest /
    /// cargo (Tier 2). Set by the view filter; the game loop uses it to decide
    /// whether a rival's freight entries may be shown.
    #[serde(default)]
    pub revealed: bool,
    /// §TCA: OWNER-ONLY — whether this (blockading) fleet is set to engage
    /// Authority freight. `None` on anyone else's ghost, like `posture`.
    #[serde(default)]
    pub engage_freight: Option<bool>,
    /// §blockade-running: OWNER-ONLY, on a freighter fleet — whether it runs a
    /// held blockade at its destination rather than waiting outside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_blockades: Option<bool>,
    /// OWNER-ONLY: the system whose held blockade this delivery is waiting
    /// outside (on the standoff ring, its delivery still open).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_outside: Option<EntityId>,
    /// §raid-order: OWNER-ONLY — this blockading fleet is on a RAID: it brings
    /// its loot home when its holds fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raiding: Option<bool>,
    /// §player-styles (v80): a repeating raid's trip, this one included.
    /// Owner-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_trip: Option<u32>,
    /// §light-delayed controls: owner-only served transit setting. Unlike a
    /// local button latch, this can reconcile a pending request after the
    /// fleet's own report returns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transit: Option<sim::ship::TransitMode>,
    /// §research-wiring Signal Libraries / ECM Emitters: the size estimate as a
    /// range one step finer or coarser than `count_class` (low, high; None:
    /// open-ended). Absent: read `count_class`.
    #[serde(default)]
    pub count_range: Option<(u32, Option<u32>)>,
    /// §research-wiring Traffic Analysis: what an unrevealed freighter carries:
    /// goods, personnel, modules or empty.
    #[serde(default)]
    pub manifest_class: Option<String>,
    /// §research-wiring Wake Analysis: its tank and the way it came.
    #[serde(default)]
    pub wake: Option<WakeView>,
    /// §research-wiring Standing Watch: a lost contact's dead-reckoned
    /// estimate — seconds since its last fix.
    #[serde(default)]
    pub tracked: Option<f64>,
    /// §research-wiring Predictive Plots: where a moving rival is now if it
    /// held course since its light left it.
    #[serde(default)]
    pub projected: Option<Vec2>,
}

/// Messages pushed by the server to a single player's connection.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    /// The authenticated account has not founded a corporation in this galaxy;
    /// `profile` is the home world it has been dealt there.
    FoundingOffer { profile: sim::settlement::FoundingProfile },
    /// Sent once, immediately after a successful join.
    Welcome {
        player_id: PlayerId,
        name: String,
        /// The wire protocol version ([`PROTOCOL_VERSION`]) — lets the client
        /// detect a stale build against a newer server.
        protocol_version: u32,
        /// Sim tick rate (Hz) — lets the client display time correctly.
        tick_hz: u32,
        /// Sim seconds advanced per wall second. `1` is production pacing;
        /// playtests default to `4` so two strategic hours fit in ~30 minutes.
        pacing_scale: f64,
        tick: u64,
        sim_time: f64,
        galaxy: GalaxyInfo,
        /// §perf Part B: the charter band ladder — a static constant table
        /// (title, standing at/below which it applies), sent once here instead
        /// of inside every 10 Hz View.
        charter_ladder: Vec<(&'static str, f64)>,
        /// §standing-ladder (R3): the grades above full standing.
        grade_ladder: Vec<(&'static str, f64)>,
        /// §perf Part B: the static research programme catalog (names, blurbs,
        /// board topology, costs) — the game's public rulebook, identical for
        /// everyone. The View carries only the per-node dynamic slice.
        research_catalog: Vec<ProgrammeInfo>,
    },

    /// The public star chart CHANGED after this client's Welcome: a join past
    /// the pre-generated home-slot pool MINTS a brand-new home system mid-run,
    /// and every connected client's galaxy snapshot predates it — without this
    /// refresh the new star is invisible and unselectable (not least to its own
    /// owner). Replaces the client's `galaxy.systems` wholesale; public
    /// geography only, so there is nothing to fog.
    GalaxyUpdate { systems: Vec<SystemInfo> },

    /// The player's per-tick delayed/fogged view of the world, computed from
    /// THEIR command center (§6). Every player receives a *different* one; none
    /// receives true positions, another player's view, or any presence
    /// information faster than light. (Deliberately carries no global
    /// player-count — that would leak join/leave instantly; the client derives
    /// "corps in view" from the fair, light-delayed ghosts it can see.)
    View {
        tick: u64,
        sim_time: f64,
        /// This player's command center — the origin of their light-cone.
        command_center: Vec2,
        /// Home anchors; each owner is light-gated (see [`AnchorView`]).
        anchors: Vec<AnchorView>,
        /// Arrived site reports for this CC. Ownership does not bypass delay;
        /// private inventory additionally requires reported ownership.
        systems: Vec<SystemStateView>,
        /// Ships as delayed ghosts from this player's vantage.
        ghosts: Vec<GhostView>,
        /// The viewer's own officer ledger. Remote progression is sourced from
        /// `ghosts`, never current truth; reserve officers are at the home CC.
        captains: Vec<CaptainRosterView>,
        captain_capacity: u32,
        /// Newly/continuingly observable rival departure tombstones. The client
        /// retains each unique event briefly as a fading historical marker.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        jump_departures: Vec<JumpDepartureView>,
        /// Arrived infrastructure reports; new construction and destruction
        /// cross the same light gate even for the structure's owner.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        emplacements: Vec<EmplacementView>,
        /// Interdiction fields whose light has arrived. Public: everyone sees
        /// a field, wherever it is.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        interdiction_fields: Vec<InterdictionFieldView>,
        /// The hub ticker, light-delayed (§9).
        market: MarketView,
        /// Arrived Market Hub account report, or explicit report_pending.
        wallet: WalletView,
        /// §TCA Phase 2: the player's OWN charter standing and band. Owner-only —
        /// rivals learn of offenses only through public citations, never by
        /// reading a corporation's record.
        charter: CharterView,
        /// The viewer's server-authoritative opening arc and founder safety.
        founding: FoundingView,
        /// §founding R10: the charter ledger, once it opens (the first
        /// guarded export). Owner-only.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        charter_ledger: Option<CharterLedgerView>,
        /// §leave (R4): the viewer's own leave. Owner-only.
        leave: LeaveView,
        /// §player-styles: what raids and blockades took from the viewer
        /// today, against its daily loot cap. Owner-only.
        raid_losses: RaidLossView,
        /// §merchant: the traveling merchant serving the viewer's home.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        merchant: Option<MerchantView>,
        /// §TCA: the Market Hub freight desk — timetable, terms per owned
        /// destination, and the player's OWN shipment queue. Owner-only, fresh
        /// (it is the player's own administration, like the wallet).
        freight: FreightView,
        /// The player's own fleet doctrine (§16) — fresh private policy (like the
        /// wallet), so the client can display and edit it.
        doctrine: FleetDoctrine,
        /// The player's own in-flight ORDER LIFECYCLES (§order-lifecycle) — the
        /// two exact timestamps per pending order that let the client tick down IN
        /// TRANSIT → AWAITING ECHO. OWNER-ONLY private command data (like the
        /// wallet); a rival's view carries none of it.
        pending_orders: Vec<PendingOrderView>,
        /// Ongoing BATTLES visible to this player (§battles-take-time) — strictly
        /// light-gated; a third-party observer sees them only by their own light.
        battles: Vec<BattleView>,
        /// §syndicates Part 1: the viewer's OWN syndicate roster (fresh private
        /// state, like the wallet), or `None` if unaffiliated. Never a rival's.
        /// Boxed so this (already the largest) View variant stays lean; serde is
        /// transparent through the `Box`, so the wire JSON is unchanged.
        #[serde(default)]
        syndicate: Option<Box<SyndicateView>>,
        /// §syndicates Part 1: pending invitations the viewer may accept.
        #[serde(default)]
        syndicate_invites: Vec<SyndicateInviteView>,
        /// Repeatable work and strategic objectives, each at the latest report
        /// that has actually reached this corporation.
        #[serde(default)]
        operations: Vec<OperationView>,
        /// §contract-board: None until the charter is fulfilled.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        board: Option<ContractBoardView>,
        /// §signals: None until signals open (after the den).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signals: Option<SignalsView>,
        /// Arrived site snapshots only; never the procedural hidden-site table.
        #[serde(default)]
        exploration_sites: Vec<sim::sites::SiteReport>,
        /// Capability-derived chapter: guidance, not a hard level gate.
        midgame_stage: sim::MidgameStage,
        /// Bilateral relations and arrived treaty offers. Boxed for View size.
        #[serde(default)]
        diplomacy: Option<Box<DiplomacyView>>,
        /// §research R6: the viewer's OWN corporation research picture (owner-only,
        /// like the wallet). Present for every real corporation regardless of
        /// syndicate membership. Boxed to keep this variant lean.
        #[serde(default)]
        research: Option<Box<ResearchView>>,
    },

    /// §perf Part B: the SLOW-MOVING per-player sections that used to ride every
    /// 10 Hz View — standing orders, retained battle/capture reports, private
    /// discovery notes and published rankings. Sent ONLY when a section's content
    /// changed (signature-gated, the timeline_sent pattern), on the RELIABLE
    /// discrete lane (the View's watch channel may drop frames for a slow
    /// client, which would lose a once-per-change section forever). A present
    /// field REPLACES the client's copy; an absent field means "unchanged". A
    /// fresh connection's first broadcast carries every section (empty signatures).
    Sections {
        #[serde(skip_serializing_if = "Option::is_none")]
        exploration_journal: Option<Vec<sim::sites::JournalEntry>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        standing_orders: Option<Vec<StandingOrder>>,
        /// §fuel-overhaul: the corp's fuel depot rules (sent with standing orders).
        #[serde(skip_serializing_if = "Option::is_none")]
        depot_rules: Option<Vec<sim::industry::DepotRule>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        battle_reports: Option<Vec<BattleReportView>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        capture_reports: Option<Vec<CaptureReportView>>,
        /// §rankings: the PUBLISHED leaderboard — the same snapshot for every
        /// player (public by design), taken on the ledger close; it only changes
        /// on a close, which is exactly when it re-sends.
        #[serde(skip_serializing_if = "Option::is_none")]
        rankings: Option<Vec<RankingRow>>,
        /// §syndicate-projects (R6): the syndicates' table, sent with it.
        #[serde(skip_serializing_if = "Option::is_none")]
        syndicate_rankings: Option<Vec<sim::rankings::SyndicateRankingRow>>,
        /// §social (R5): the syndicate channel, mail and member presence.
        #[serde(skip_serializing_if = "Option::is_none")]
        social: Option<crate::social::SocialView>,
        /// §season (v75): the season as this corporation has heard it.
        #[serde(skip_serializing_if = "Option::is_none")]
        season: Option<SeasonView>,
    },

    /// §perf Part A: incremental battle-record delivery (was: every record's full
    /// arrived prefix re-shipped inside every View). Rides the RELIABLE discrete
    /// lane (the bounded mpsc, like Timeline/Report) — the View's watch channel is
    /// last-write-wins and may drop frames for a slow client, which would lose a
    /// delta forever. The per-connection cursor advances only when the send
    /// succeeds, so a full queue simply retries next broadcast. `removed` names
    /// records this connection should drop: pruned server-side, or (bucket
    /// fidelity) the viewer's sensor coverage of the site lapsed — exactly the
    /// records that vanished from the old full-set View. A record that becomes
    /// visible again is re-sent in full (fresh cursor), as before.
    BattleRecords {
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        updates: Vec<BattleRecordUpdate>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        removed: Vec<EntityId>,
    },

    /// §ground G2: incremental LANDING-record delivery. Same reliable lane and
    /// same cursor discipline as `BattleRecords`, for the same reasons.
    GroundRecords {
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        updates: Vec<GroundRecordUpdate>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        removed: Vec<EntityId>,
    },

    /// A delayed raid report (§8) — arrives on the recipient's own clock.
    Report { report: RaidReport },

    /// The player's check-in timeline (§16, Layer 3) — the retained digest of what
    /// became OBSERVABLE to them, buffered across disconnects. Sent on connect (the
    /// "welcome back" digest) and again whenever it grows. `away_since` is the
    /// sim-time they were last online, so the client can split "while you were
    /// away" from earlier entries. Awareness only — never new information, never
    /// faster than light.
    Timeline {
        entries: Vec<TimelineEntry>,
        away_since: f64,
        /// §welcome (R9): the report on return, on the connect digest only,
        /// after an absence of ten minutes or more.
        #[serde(skip_serializing_if = "Option::is_none")]
        welcome: Option<crate::welcome::WelcomeBackView>,
    },

    /// Economy news for this player (§9): a buy settled, a delivery arrived, a
    /// sell was dispatched or cleared.
    Trade { trade: TradeEvent },
    Transactions {
        player_id: PlayerId,
        request_id: u32,
        before: Option<u64>,
        entries: Vec<crate::transactions::TransactionEntry>,
        next_before: Option<u64>,
        since: f64,
    },
    TransactionRecorded {
        player_id: PlayerId,
        entry: crate::transactions::TransactionEntry,
    },

    /// Feedback for an order the player just issued — the OUTBOUND command in
    /// flight (§6, "commanding into the past"). Sent immediately to the issuing
    /// player, carrying authoritative sim-times:
    ///   * `depart_time` — the order leaves the command center;
    ///   * `arrive_time` — the routed outbound signal reaches the moving meeting
    ///     point projected from the ship's currently observed ghost.
    ///
    /// This is the one thing the MAP can't show — your command crossing space,
    /// not yet arrived. The ship's *reaction* needs no signal: the player simply
    /// sees the ghost change course on the map when its light arrives (the map IS
    /// the inbound channel). The route is calculated only from the player's
    /// OBSERVED ghost position and velocity, never the hidden true fleet, and
    /// its own hop total supplies the clock.
    CommandSignal {
        order_id: u64,
        ship_id: EntityId,
        depart_time: f64,
        arrive_time: f64,
        /// The path the order actually flies — each hop's
        /// position with its arrival as a 0..1 FRACTION of the whole window,
        /// so the comet lands exactly at `arrive_time`. Empty means the normal
        /// single straight run and lets the client use its direct fallback.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hops: Vec<SignalHopView>,
    },

    /// Outbound feedback for a remote instruction that does not enter the
    /// fleet-movement order lifecycle: Market Hub business and immediate fleet
    /// administration. It means only "the instruction left command"; it is not
    /// an acceptance receipt. `fleet_id` lets the client bend the final leg onto
    /// the same served/eased fleet glyph, while `target_pos` is the player-known
    /// meeting point and the fixed endpoint for Hub instructions.
    CommandChevron {
        #[serde(skip_serializing_if = "Option::is_none")]
        fleet_id: Option<EntityId>,
        target_pos: Vec2,
        depart_time: f64,
        arrive_time: f64,
    },

    /// Arrived evidence that an order is in force. This is emitted only by the
    /// sim's confirming response/re-entry event, never by an estimate expiring.
    OrderConfirmed {
        order_id: u64,
        ship_id: EntityId,
        kind: OrderKind,
    },

    /// A projected engagement estimate the player asked for (§FLEETS Part 3).
    EngagementEstimate(EngagementEstimate),

    /// A protocol-level error (e.g. a malformed first message).
    Error { message: String },

    /// §settle-near-friends (R7): the answer to `RedeemInvite`.
    InviteStatus {
        ok: bool,
        /// The inviting corporation.
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        /// Its syndicate, which the newcomer joins seat permitting.
        #[serde(skip_serializing_if = "Option::is_none")]
        syndicate: Option<String>,
        /// The held home system.
        #[serde(skip_serializing_if = "Option::is_none")]
        near: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
}

/// Legacy deterministic IDs for simulation fixtures ONLY. Live accounts have
/// random, durable PostgreSQL IDs; a public corporation name is not a login.
#[cfg(test)]
pub fn player_id_from_name(name: &str) -> PlayerId {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in name.trim().to_lowercase().bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    // Guard the reserved NEUTRAL sentinel ids (PIRATE, TCA): a real name that
    // happened to hash onto one would be mistaken for the faction that owns pirate
    // packs / Authority freighters. A 1-in-2^64 event, but the sentinels' whole
    // safety rests on "no corporation ever has this id", so we make it certain.
    while hash == PlayerId::PIRATE.0 || hash == PlayerId::TCA.0 {
        hash = hash.wrapping_add(1);
    }
    PlayerId(hash)
}

#[cfg(test)]
mod wire_contract {
    use super::*;

    #[test]
    fn join_view_cadence_is_backward_compatible() {
        let old: ClientMsg = serde_json::from_str(r#"{"type":"Join","name":"Meridian"}"#)
            .expect("an older Join without a cadence keeps working");
        let mobile: ClientMsg =
            serde_json::from_str(r#"{"type":"Join","name":"Meridian","view_hz":5}"#)
                .expect("a mobile Join carries its requested cadence");

        assert!(matches!(old, ClientMsg::Join { view_hz: None, .. }));
        assert!(matches!(
            mobile,
            ClientMsg::Join {
                view_hz: Some(5),
                ..
            }
        ));
    }

    #[test]
    fn jump_ship_parses_off_the_wire() {
        let raw = r#"{"type":"JumpShip","ship_id":"33","dest":{"x":42000.0,"y":-7000.0}}"#;
        let msg: ClientMsg =
            serde_json::from_str(raw).expect("the client's literal jump message must parse");
        match msg {
            ClientMsg::JumpShip { ship_id, dest } => {
                assert_eq!(ship_id, sim::EntityId(33));
                assert_eq!(dest, sim::Vec2::new(42_000.0, -7_000.0));
            }
            other => panic!("parsed into the wrong variant: {other:?}"),
        }
    }

    /// §emplacements: the EXACT JSON the client's build-here button sends must
    /// parse. This variant was missing at launch — the client spoke, the
    /// server answered "malformed message", and the readout claimed success. A
    /// wire message the enum can't parse is invisible in play, so the contract
    /// is pinned here with the client's literal bytes.
    /// §emplacements: the demolish order's literal bytes must parse. Same
    /// discipline as the build message — a ClientMsg variant that does not exist
    /// fails SILENTLY in play (the socket answers "malformed" into a dialog
    /// nobody is looking at), so the contract is pinned here.
    #[test]
    fn demolish_emplacement_parses_off_the_wire() {
        let raw = r#"{"type":"DemolishEmplacement","fleet":"34","target":"41"}"#;
        let msg: ClientMsg =
            serde_json::from_str(raw).expect("the client's literal message must parse");
        match msg {
            ClientMsg::DemolishEmplacement { fleet, target } => {
                assert_eq!(fleet, sim::EntityId(34));
                assert_eq!(target, sim::EntityId(41));
            }
            other => panic!("parsed into the wrong variant: {other:?}"),
        }
    }

    #[test]
    fn build_emplacement_parses_off_the_wire() {
        let raw = r#"{"type":"BuildEmplacement","builder":"33","emplacement":"deep_space_sensor"}"#;
        let msg: ClientMsg =
            serde_json::from_str(raw).expect("the client's literal message must parse");
        match msg {
            ClientMsg::BuildEmplacement {
                builder,
                emplacement,
            } => {
                assert_eq!(builder, sim::EntityId(33));
                assert_eq!(emplacement, sim::emplace::EmplacementKind::DeepSpaceSensor);
            }
            other => panic!("parsed into the wrong variant: {other:?}"),
        }
    }

    /// The View's order queue is consumed directly by the rebuilt ship panel.
    /// Pin the literal list shape—including identity and subject—so silently
    /// dropping back to one anonymous lifecycle cannot masquerade as a valid
    /// payload.
    #[test]
    fn pending_order_queue_parses_off_the_wire() {
        let raw = r#"[
            {"id":17,"fleet_id":"33","issued_at":10.0,"arrives_at":18.0,"response_at":26.0,"kind":"move","dest":{"x":287450.0,"y":35020.0},"intent_path":[{"x":100.0,"y":200.0},{"x":287450.0,"y":35020.0}]},
            {"id":18,"fleet_id":"33","issued_at":15.0,"arrives_at":22.0,"response_at":30.0,"kind":"construct","dest":{"x":900.0,"y":1200.0},"emplacement":"deep_space_sensor"},
            {"id":19,"fleet_id":"33","issued_at":16.0,"arrives_at":23.0,"response_at":31.0,"kind":"attack","target_id":"44"}
        ]"#;
        let queue: Vec<PendingOrderView> =
            serde_json::from_str(raw).expect("the client's literal queue shape must parse");
        assert_eq!(queue.len(), 3);
        assert_eq!(queue[0].id, 17);
        assert_eq!(queue[0].fleet_id, sim::EntityId(33));
        assert_eq!(queue[0].arrives_at, 18.0);
        assert_eq!(queue[0].response_at, 26.0);
        assert_eq!(queue[0].dest, Some(sim::Vec2::new(287_450.0, 35_020.0)));
        assert_eq!(
            queue[0].intent_path,
            vec![
                sim::Vec2::new(100.0, 200.0),
                sim::Vec2::new(287_450.0, 35_020.0),
            ]
        );
        assert_eq!(queue[1].id, 18);
        assert_eq!(queue[1].kind, sim::event::OrderKind::Construct);
        assert!(queue[1].intent_path.is_empty());
        assert_eq!(
            queue[1].emplacement,
            Some(sim::emplace::EmplacementKind::DeepSpaceSensor)
        );
        assert_eq!(queue[2].kind, sim::event::OrderKind::Attack);
        assert_eq!(queue[2].target_id, Some(sim::EntityId(44)));
        assert!(queue[2].intent_path.is_empty());
        let encoded = serde_json::to_value(&queue[0]).unwrap();
        assert!(encoded.get("arrives_at").is_some());
        assert!(encoded.get("response_at").is_some());
        assert!(encoded.get("response_on_reentry").is_none());
        assert!(encoded.get("intent_path").is_some());
        assert!(encoded.get("projected").is_none());
        assert!(encoded.get("delivered_at").is_none());
        assert!(encoded.get("echo_at").is_none());
    }

    #[test]
    fn lost_orders_confirmation_and_dismissal_round_trip_off_the_wire() {
        let raw = r#"{"type":"DismissLostOrder","order_id":17}"#;
        assert!(matches!(
            serde_json::from_str::<ClientMsg>(raw).unwrap(),
            ClientMsg::DismissLostOrder { order_id: 17 }
        ));

        let lost: PendingOrderView = serde_json::from_str(
            r#"{"id":17,"fleet_id":"33","issued_at":10.0,"arrives_at":18.0,"response_at":26.0,"kind":"move","lost":true,"loss_relay":"41","loss_break":{"x":12.0,"y":34.0}}"#,
        )
        .unwrap();
        assert!(lost.lost);
        assert_eq!(lost.loss_relay, Some(sim::EntityId(41)));
        assert_eq!(lost.loss_break, Some(sim::Vec2::new(12.0, 34.0)));

        let confirmed = ServerMsg::OrderConfirmed {
            order_id: 17,
            ship_id: sim::EntityId(33),
            kind: sim::event::OrderKind::Move,
        };
        let encoded = serde_json::to_value(confirmed).unwrap();
        assert_eq!(encoded["type"], "OrderConfirmed");
        assert_eq!(encoded["order_id"], 17);
    }
}

/// §season (v75): the season as one corporation has heard it from the Hub.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SeasonView {
    /// Galaxy maturity, 0–1, at the census heard.
    pub maturity: f64,
    pub grip: sim::season::Grip,
    /// When that census was taken (sim time).
    pub census_at: f64,
    /// Galactic News Network items heard, newest first.
    pub news: Vec<NewsView>,
    /// §season phase 3: the galaxy's Resolve and the Authority's war fleet
    /// (hulls), as of the census heard.
    pub galaxy_resolve: f64,
    pub war_fleet: u32,
    /// Crackdowns heard and still in force, or with evidence still to find.
    pub crackdowns: Vec<CrackdownView>,
    /// §season phase 4: relics awake (heard), with your resonance on yours.
    pub relics: Vec<RelicView>,
    /// Bearings you and your syndicate drew (their worlds are never served).
    pub bearings: Vec<BearingView>,
    /// Sealed worlds your circle found, and every one landed on.
    pub precursors: Vec<PrecursorView>,
    /// The spy's leak: the Authority knows then.
    pub war_at: Option<f64>,
    /// You are at war with the Authority (since then).
    pub rebel_since: Option<f64>,
    /// Corporations at war with the Authority (heard).
    pub rebels: Vec<PlayerId>,
    /// The race began then (the first sealed world found).
    pub race_since: Option<f64>,
    /// §season phase 5: every Keystone's level and holder (public).
    pub keystones: Vec<KeystoneView>,
    /// Your syndicate's Keystone: what its next level takes, and its powers.
    pub keystone: Option<MyKeystoneView>,
    /// The season ends then if no Seal fires (the race, or the backstop).
    pub ends_at: Option<f64>,
    pub ending: Option<EndingView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeystoneView {
    pub system: EntityId,
    pub holder: Option<PlayerId>,
    pub holder_name: Option<String>,
    pub level: u32,
    /// The Seal fires then, if it holds.
    pub seal_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MyKeystoneView {
    pub system: EntityId,
    pub level: u32,
    /// The next level's goods: (good, needed, paid).
    pub basket: Vec<(Commodity, f64, f64)>,
    pub relics_needed: usize,
    pub relics_held: usize,
    /// The four powers are yours.
    pub powers: bool,
    /// Gate Step's gates: the Keystone world and the relics held.
    pub gates: Vec<EntityId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndingView {
    /// "sovereign" or "charter_holds".
    pub kind: String,
    pub at: f64,
    pub holder: Option<String>,
    pub system: Option<EntityId>,
    pub winners: Vec<String>,
    /// You are among the winners (or the Sovereign).
    pub won: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelicView {
    pub system: EntityId,
    pub awake_at: f64,
    /// Your attunement, 0–1, on a relic you hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance: Option<f64>,
    /// You drew its bearing.
    pub drawn: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BearingView {
    pub relic: EntityId,
    pub from: Vec2,
    pub angle: f64,
    /// Half-width of the wedge (radians).
    pub spread: f64,
    pub by: PlayerId,
    pub at: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrecursorView {
    pub system: EntityId,
    pub body: u32,
    pub found_at: Option<f64>,
    pub landed_by: Option<PlayerId>,
    pub guarded: bool,
}

/// §season phase 3: a crackdown as heard. Whether it was staged is never
/// served, only whether it was exposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrackdownView {
    pub id: u64,
    pub kind: sim::season::CrackdownKind,
    pub at: f64,
    pub until: f64,
    pub pos: Vec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<PlayerId>,
    /// Evidence at the scene until then (0: none).
    pub evidence_until: f64,
    pub exposed: bool,
    /// This corporation's Scouts already read it.
    pub read: bool,
}

/// §season (v75): one GNN or Free Signal item as heard at a command center.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsView {
    pub id: u64,
    /// When its light reached this command center.
    pub heard_at: f64,
    pub source: sim::season::NewsSource,
    pub kind: sim::season::NewsKind,
    pub headline: String,
    /// The story names the viewer's corporation.
    pub about_you: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<EntityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fleet: Option<EntityId>,
}

/// GNN items served to each corporation.
pub const NEWS_SERVED: usize = 40;

impl SeasonView {
    /// The season `player` at `cc` has heard by `now`.
    pub fn heard(world: &sim::World, player: PlayerId, cc: Vec2) -> SeasonView {
        let (hub, c, now) = (world.hub, world.config.c, world.time);
        let census = world.season.census_for(cc, hub, c, now);
        let lag = sim::transit::delay(hub, cc, c);
        let heard_at = now - lag;
        let c = world.config.c;
        let circle = world.circle(player);
        let sys_pos = |id: EntityId| world.systems.iter().find(|s| s.id == id).map(|s| s.pos);
        let holder = |id: EntityId| world.systems.iter().find(|s| s.id == id).and_then(|s| s.owner);
        let relics = world.season.relics.iter()
            .filter_map(|(id, r)| {
                let awake = r.awake_at?;
                let pos = sys_pos(*id)?;
                (awake + sim::transit::delay(pos, cc, c) <= now).then(|| RelicView {
                    system: *id, awake_at: awake,
                    resonance: (holder(*id) == Some(player)).then(|| if r.attuner == Some(player) { r.resonance } else { 0.0 }),
                    drawn: r.bearings_to.contains(&player),
                })
            }).collect();
        let bearings = world.season.bearings.iter()
            .filter(|b| circle.contains(&b.corp) && b.at + sim::transit::delay(b.from, cc, c) <= now)
            .map(|b| BearingView { relic: b.relic, from: b.from, angle: b.angle, spread: sim::season::BEARING_SPREAD, by: b.corp, at: b.at })
            .collect();
        let precursors = world.season.precursors.iter()
            .filter_map(|(id, p)| {
                let pos = sys_pos(*id)?;
                let heard = |t: f64| t + sim::transit::delay(pos, cc, c) <= now;
                let found_at = p.found.iter().filter(|(who, at)| circle.contains(who) && heard(*at)).map(|(_, at)| *at)
                    .min_by(|a, b| a.total_cmp(b));
                let landed = p.landed.filter(|(_, at)| heard(*at));
                (found_at.is_some() || landed.is_some()).then(|| PrecursorView {
                    system: *id, body: p.body, found_at, landed_by: landed.map(|(who, _)| who),
                    guarded: world.precursor_guarded(*id),
                })
            }).collect();
        let name_of = |p: PlayerId| world.players.get(&p).map(|c| c.name.clone());
        let keystones = world.season.precursors.iter()
            .filter_map(|(id, p)| {
                let k = p.keystone.as_ref().filter(|k| k.level > 0)?;
                let holder = world.systems.iter().find(|s| s.id == *id).and_then(|s| s.owner);
                Some(KeystoneView { system: *id, holder, holder_name: holder.and_then(name_of), level: k.level,
                    seal_at: k.charge_since.map(|t| t + sim::season::CHARGE_S) })
            }).collect();
        let keystone = world.season.precursors.iter()
            .find(|(id, p)| (p.found.iter().any(|(f, _)| circle.contains(f)) || p.landed.is_some())
                && world.systems.iter().any(|s| s.id == **id && s.owner.is_some_and(|o| circle.contains(&o))))
            .map(|(id, p)| {
                let level = p.keystone.as_ref().map_or(0, |k| k.level);
                let scale = (world.home_slots.len() as f64 / 20.0).max(0.25);
                let next = (level + 1).min(sim::season::KEYSTONE_LEVELS);
                let paid = |c: Commodity| p.keystone.as_ref().and_then(|k| k.paid.get(&c)).copied().unwrap_or(0.0);
                let held: Vec<EntityId> = world.season.relics.iter()
                    .filter(|(rid, r)| r.awake_at.is_some()
                        && world.systems.iter().any(|s| s.id == **rid && s.owner.is_some_and(|o| circle.contains(&o))))
                    .map(|(rid, _)| *rid).collect();
                let powers = world.keystone_powers(player);
                MyKeystoneView {
                    system: *id, level,
                    basket: if level >= sim::season::KEYSTONE_LEVELS { Vec::new() } else {
                        sim::season::keystone_basket(next, scale).into_iter().map(|(c, n)| (c, n, paid(c))).collect()
                    },
                    relics_needed: sim::season::keystone_relics(next),
                    relics_held: held.len(),
                    powers,
                    gates: if powers { std::iter::once(*id).chain(held).collect() } else { Vec::new() },
                }
            });
        let ends_at = world.season.first_found_at.map(|t| t + sim::season::RACE_S)
            .or_else(|| world.season.started_at.map(|t| t + sim::season::BACKSTOP_S));
        let ending = world.season.ended.as_ref().map(|e| match e {
            sim::season::Ending::Sovereign { holder, system, at } => EndingView {
                kind: "sovereign".into(), at: *at, holder: name_of(*holder), system: Some(*system),
                winners: name_of(*holder).into_iter().collect(), won: circle.contains(holder),
            },
            sim::season::Ending::CharterHolds { at, winners } => EndingView {
                kind: "charter_holds".into(), at: *at, holder: None, system: None,
                winners: winners.iter().filter_map(|w| name_of(*w)).collect(), won: winners.contains(&player),
            },
        });
        SeasonView {
            keystones,
            keystone,
            ends_at,
            ending,
            relics,
            bearings,
            precursors,
            war_at: world.season.leaks.iter().find(|l| circle.contains(&l.corp)).map(|l| l.at),
            rebel_since: world.season.rebels.get(&player).copied(),
            rebels: world.season.rebels.iter().filter(|(_, at)| **at <= heard_at).map(|(p, _)| *p).collect(),
            race_since: world.season.first_found_at.filter(|at| *at <= heard_at),
            galaxy_resolve: world.season.galaxy_resolve,
            war_fleet: world.season.war_fleet,
            crackdowns: world.season.crackdowns.iter()
                .filter(|c| c.at <= heard_at && (c.until > now || c.evidence_until > now))
                .map(|c| CrackdownView {
                    id: c.id, kind: c.kind, at: c.at, until: c.until, pos: c.pos, system: c.system, target: c.target,
                    evidence_until: c.evidence_until, exposed: c.exposed, read: c.investigated.contains(&player),
                }).collect(),
            maturity: census.map_or(0.0, |r| r.maturity),
            grip: census.map_or_else(Default::default, |r| r.grip),
            census_at: census.map_or(0.0, |r| r.at),
            news: world.season.news_for(cc, hub, c, now, NEWS_SERVED).into_iter().map(|n| NewsView {
                id: n.id,
                heard_at: n.at + lag,
                source: n.source,
                kind: n.kind,
                headline: n.headline.clone(),
                about_you: n.corps.contains(&player),
                system: n.system,
                fleet: n.fleet,
            }).collect(),
        }
    }

    /// A cheap change signature: the census heard and the newest item.
    pub fn signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.census_at.to_bits().hash(&mut h);
        (self.grip as u8).hash(&mut h);
        self.news.first().map(|n| n.id).hash(&mut h);
        self.news.len().hash(&mut h);
        self.war_fleet.hash(&mut h);
        self.galaxy_resolve.to_bits().hash(&mut h);
        for c in &self.crackdowns {
            (c.id, c.until.to_bits(), c.evidence_until.to_bits(), c.exposed, c.read).hash(&mut h);
        }
        for r in &self.relics {
            (r.system, r.resonance.map(|x| (x * 100.0) as u32), r.drawn).hash(&mut h);
        }
        (self.bearings.len(), self.precursors.len(), self.rebels.len()).hash(&mut h);
        for p in &self.precursors {
            (p.system, p.landed_by, p.guarded).hash(&mut h);
        }
        (self.war_at.map(f64::to_bits), self.rebel_since.map(f64::to_bits), self.race_since.map(f64::to_bits)).hash(&mut h);
        for k in &self.keystones {
            (k.system, k.level, k.seal_at.map(f64::to_bits)).hash(&mut h);
        }
        if let Some(k) = &self.keystone {
            (k.system, k.level, k.relics_held, k.powers).hash(&mut h);
            for (_, need, paid) in &k.basket {
                (need.to_bits(), paid.to_bits()).hash(&mut h);
            }
        }
        (self.ending.as_ref().map(|e| e.at.to_bits()), self.ends_at.map(f64::to_bits)).hash(&mut h);
        h.finish()
    }
}

#[cfg(test)]
mod raid_over_tests {
    use super::*;

    #[test]
    fn the_raider_is_over_the_colony_only_once_its_arrival_is_heard() {
        let mut w = sim::World::new(sim::SimConfig::for_players(4545, 4));
        let me = PlayerId(7202);
        w.step(&[sim::Command::AddPlayer { id: me, name: "Founder".into() }]);
        assert!(!raid_over_heard(&w, me, w.time), "no raid, nothing over the colony");
        let home = w.players[&me].home_system.and_then(|id| w.systems.iter().find(|s| s.id == id)).unwrap().pos;
        let arrived = w.time;
        w.players.get_mut(&me).unwrap().founding.raid = Some(sim::founding::FoundingRaid {
            phase: sim::founding::RaidPhase::Inbound, base: home, lurk: home, arrived_at: None,
            allowance: Default::default(), stolen: Default::default(), loading_credit: 0.0,
        });
        assert!(!raid_over_heard(&w, me, arrived + 10_000.0), "inbound: still closing, however long");
        w.players.get_mut(&me).unwrap().founding.raid.as_mut().unwrap().arrived_at = Some(arrived);
        let light = sim::transit::delay(home, w.players[&me].command_center, w.config.c);
        assert!(!raid_over_heard(&w, me, arrived + light * 0.5) || light == 0.0, "its arrival is still on its way home");
        assert!(raid_over_heard(&w, me, arrived + light + 1e-6), "heard: over the colony");
    }
}

#[cfg(test)]
mod licence_view_tests {
    use super::*;
    use sim::licence::LicenceRefusal;

    #[test]
    fn a_licence_shows_held_only_when_the_receipt_reaches_home() {
        let mut w = sim::World::new(sim::SimConfig::for_players(4343, 4));
        let me = PlayerId(7101);
        w.step(&[sim::Command::AddPlayer { id: me, name: "Licensee".into() }]);
        let heard = |w: &sim::World, now: f64| LicenceView::heard(w, me, 100.0, now);
        let destroyer = |views: Vec<LicenceView>| views.into_iter().find(|v| v.hull == ShipKind::Destroyer).unwrap();
        let before = destroyer(heard(&w, w.time));
        assert!(!before.held);
        assert_eq!(before.refusal, Some(LicenceRefusal::NeedsResearch), "research is the skill, first");
        assert!((before.price - sim::licence::price(ShipKind::Destroyer, 100.0)).abs() < 1e-6);
        assert_eq!(heard(&w, w.time).len(), sim::licence::licensed_hulls().len());

        w.players.get_mut(&me).unwrap().research.completed.insert("hull_line_iv_destroyer".into());
        let for_sale = destroyer(heard(&w, w.time));
        assert!(!for_sale.held && for_sale.refusal.is_none(), "researched: for sale");

        let sold = w.time;
        w.players.get_mut(&me).unwrap().licences.insert(ShipKind::Destroyer, sold);
        let receipt = sim::transit::delay(w.hub, w.players[&me].command_center, w.config.c);
        assert!(receipt > 0.0);
        assert!(!destroyer(heard(&w, sold + receipt * 0.5)).held, "the receipt is still on its way");
        let held = destroyer(heard(&w, sold + receipt + 1e-6));
        assert!(held.held && held.refusal.is_none());

        let titan = heard(&w, w.time).into_iter().find(|v| v.hull == ShipKind::Titan).unwrap();
        assert!((titan.price - sim::licence::price(ShipKind::Titan, sim::tca::TCA_COMMISSIONED_AT)).abs() < 1e-3,
            "a capital is shown at the Commissioned price");
    }
}

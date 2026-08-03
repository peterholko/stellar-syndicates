//! Stellar Syndicates — pure deterministic simulation core.
//!
//! This crate is the heart of the game and is deliberately **pure**: no I/O, no
//! async, no networking, no database (GAME_DESIGN §14). It takes a [`World`] and
//! a slice of [`Command`]s and produces the next state plus the [`Event`]s that
//! occurred. Determinism comes from a seeded RNG and a fixed timestep, which is
//! what lets the whole game be tested in isolation and (later) drive a headless
//! bot-balance harness.
//!
//! Everything that touches the outside world — sessions, WebSockets, Postgres,
//! the per-player lightspeed view filter's delivery scheduling, rendering —
//! lives in the `server` crate and the client, never here.

pub mod body;
pub mod build;
pub mod cargo;
pub mod colony;
pub mod combat;
pub mod command;
pub mod config;
pub mod detection;
pub mod doctrine;
pub mod emplace;
pub use emplace::{Emplacement, EmplacementKind};
pub mod event;
pub mod explore;
pub mod fuel;
pub mod galaxy;
pub mod ground;
pub mod ids;
pub mod market;
pub mod math;
pub mod module;
pub mod movement;
pub mod node;
pub mod pirate;
pub mod production;
pub mod rankings;
pub mod research;
pub mod rng;
pub mod ship;
pub mod specialist;
pub mod standing;
pub mod syndicate;
pub mod tactical;
pub mod tca;
pub mod transit;
pub mod world;

pub use body::{Body, BodyKind};
pub use build::{BuildJob, BuildKind, SlotPool, StructureKind};
pub use cargo::{Cargo, Commodity};
pub use colony::FoodState;
pub use combat::{
    BattleOutcomeSummary, BattleRecord, Forces, Losses, RoundNote, RoundRecord, SideRecord,
    TypedDamage, typical_forces,
};
pub use command::Command;
pub use config::{DT, SimConfig, TICK_HZ};
pub use detection::{detected as detected_by, signature as fleet_signature};
pub use doctrine::{
    DestinationInvalidPolicy, EngagementPolicy, EngagementPosture, EscortPolicy, FleetDoctrine,
    RetreatThreshold,
};
pub use event::{
    BuildRejectReason, DivertAction, Event, EventPayload, FreightStage, OrderKind,
    OrderRejectReason, RaidOutcome, TradeEvent, TradeRejectReason,
};
pub use explore::{RichnessBand, SURVEY_INITIAL_RADIUS};
pub use galaxy::{Blockade, Deposit, HomeSlot, StarSystem, claim_cost_for};
pub use ids::{EntityId, PlayerId, SyndicateId};
pub use market::{LimitOrder, Market, Side};
pub use math::Vec2;
pub use module::{DamageType, Family, Loadout, ModuleKind, weapon_family};
pub use movement::{MoveStep, advance_toward, intercept_point, pursue_step};
pub use node::{NODE_REGION_RADIUS, NODES_PER_CORP, Node, NodeBonus, node_bonus_for};
pub use pirate::{Enclave, PIRATE_ENCLAVE_COUNT};
pub use production::{Assignment, SuspendReason};
pub use rankings::{RankingCategory, RankingRow, RankingStats};
pub use rng::Rng;
pub use ship::{
    ALL_SHIP_KINDS, CountClass, DefenseEngagement, DockSite, FLAGSHIP_PRECEDENCE, Fleet,
    FleetOrder, ShipKind, TradeMission, TransitMode, fitting_points, hull_affinity,
};
pub use specialist::SpecialistKind;
pub use standing::{Endpoint, OrderStatus, StandingOrder, Trigger};
pub use syndicate::{
    DoctrineFit, SYNDICATE_MAX_FITS, SYNDICATE_MAX_FRAC, SYNDICATE_MIN_CAP, Syndicate,
    syndicate_cap,
};
pub use tca::{
    CharterStatus, FreightRun, RunLeg, Shipment, ShipmentDir, ShipmentId, charter_status,
};
pub use world::{
    AcademyContribution, BattleInfo, Corporation, Engagement, IntelSnapshot, PendingCommandView,
    World,
};

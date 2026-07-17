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
pub mod event;
pub mod explore;
pub mod fuel;
pub mod galaxy;
pub mod ids;
pub mod market;
pub mod math;
pub mod module;
pub mod movement;
pub mod node;
pub mod rankings;
pub mod research;
pub mod rng;
pub mod pirate;
pub mod production;
pub mod ship;
pub mod specialist;
pub mod standing;
pub mod syndicate;
pub mod tactical;
pub mod world;

pub use body::{Body, BodyKind};
pub use build::{BuildJob, BuildKind, SlotPool, StructureKind};
pub use cargo::{Cargo, Commodity};
pub use colony::FoodState;
pub use production::{Assignment, SuspendReason};
pub use specialist::SpecialistKind;
pub use combat::{
    attrition_tick, project_engagement, typical_forces, BattleOutcomeSummary, BattleRecord, Forces,
    Losses, RoundNote, RoundRecord, SideRecord, TypedDamage,
};
pub use module::{weapon_family, DamageType, Family, Loadout, ModuleKind};
pub use command::Command;
pub use config::{SimConfig, DT, TICK_HZ};
pub use doctrine::{
    DestinationInvalidPolicy, EngagementPolicy, EngagementPosture, EscortPolicy, FleetDoctrine,
    RetreatThreshold,
};
pub use event::{
    BuildRejectReason, DivertAction, Event, EventPayload, OrderKind, RaidOutcome, TradeEvent,
};
pub use galaxy::{claim_cost_for, Blockade, Deposit, HomeSlot, StarSystem};
pub use ids::{EntityId, PlayerId, SyndicateId};
pub use pirate::{Enclave, PIRATE_ENCLAVE_COUNT};
pub use node::{node_bonus_for, Node, NodeBonus, NODES_PER_CORP, NODE_REGION_RADIUS};
pub use rankings::{RankingCategory, RankingRow, RankingStats};
pub use explore::{RichnessBand, SURVEY_INITIAL_RADIUS};
pub use market::{LimitOrder, Market, Side};
pub use math::Vec2;
pub use movement::{advance_toward, intercept_point, pursue_step, MoveStep};
pub use rng::Rng;
pub use detection::{detected as detected_by, signature as fleet_signature};
pub use ship::{
    fitting_points, hull_affinity, CountClass, DefenseEngagement, Fleet, FleetOrder, ShipKind,
    TradeMission, TransitMode, ALL_SHIP_KINDS, FLAGSHIP_PRECEDENCE,
};
pub use standing::{Endpoint, OrderStatus, StandingOrder, Trigger};
pub use syndicate::{
    syndicate_cap, DoctrineFit, Syndicate, SYNDICATE_MAX_FITS, SYNDICATE_MAX_FRAC,
    SYNDICATE_MIN_CAP,
};
pub use world::{
    AcademyContribution, BattleInfo, Corporation, Engagement, IntelSnapshot, PendingCommandView,
    World,
};


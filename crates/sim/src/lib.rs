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

pub mod command;
pub mod config;
pub mod event;
pub mod galaxy;
pub mod ids;
pub mod math;
pub mod movement;
pub mod rng;
pub mod ship;
pub mod world;

pub use command::Command;
pub use config::{SimConfig, DT, TICK_HZ};
pub use event::{Event, EventPayload};
pub use galaxy::{HomeSlot, StarSystem};
pub use ids::{EntityId, PlayerId};
pub use math::Vec2;
pub use movement::{flip_and_burn, MoveStep};
pub use rng::Rng;
pub use ship::{Ship, ShipKind, ShipOrder};
pub use world::{Corporation, World};


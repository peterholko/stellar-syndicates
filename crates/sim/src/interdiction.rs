//! Interdiction fields. A destroyer carrying an Interdiction Field raises a
//! temporary gravity well in open space: every fleet inside it moves on
//! thrusters (a fleet in warp drops out as it enters), and no jump can start
//! or end inside it, so fast hulls catch whatever crosses it. The field holds
//! while its destroyer stays inside it, alive, for at most [`FIELD_SECONDS`];
//! the same fleet can raise another after [`FIELD_COOLDOWN_S`], and each one
//! burns [`FIELD_FUEL`] from the fleet's tanks. Fields are public: everyone
//! sees one once its light reaches them.

use serde::{Deserialize, Serialize};

use crate::{EntityId, PlayerId, Vec2};

/// Field radius (su). Wider than a system's gravity well (900), so a field
/// across a route is a real trap.
pub const FIELD_RADIUS: f64 = 1_500.0;
/// How long a field holds at most (seconds).
pub const FIELD_SECONDS: f64 = 600.0;
/// How long the fleet's generator needs between fields (from raising one).
pub const FIELD_COOLDOWN_S: f64 = 1_800.0;
/// Fuel a field burns from the fleet's tanks (a lone destroyer carries 70).
pub const FIELD_FUEL: f64 = 20.0;

/// One field in space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterdictionField {
    pub id: EntityId,
    pub owner: PlayerId,
    /// The fleet whose destroyer holds it up.
    pub fleet: EntityId,
    pub pos: Vec2,
    pub radius: f64,
    pub started_at: f64,
    pub expires_at: f64,
}

impl InterdictionField {
    pub fn contains(&self, p: Vec2) -> bool {
        p.distance(self.pos) <= self.radius
    }
}

/// Why a field ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldEnd {
    /// It ran its full time.
    Expired,
    /// Its destroyer left the field.
    Left,
    /// Its destroyer was destroyed or refitted, or its fleet changed hands.
    Lost,
}

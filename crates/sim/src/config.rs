//! Tunable constants for a galaxy. Serialisable so the exact configuration is
//! captured in snapshots and can be shipped to the client for rendering.
//!
//! Most knobs are placeholders consumed by later milestones (galaxy generation
//! in M2, the lightspeed model in M3); M1 only needs the tick cadence.

use serde::{Deserialize, Serialize};

/// Simulation tick rate. The authoritative loop advances the world this many
/// times per real second; `dt = 1 / tick_hz`.
pub const TICK_HZ: u32 = 30;

/// Fixed timestep in seconds.
pub const DT: f64 = 1.0 / TICK_HZ as f64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    /// Seed for all deterministic generation in this galaxy.
    pub seed: u64,

    /// Maximum players this galaxy is sized for. Determines the number of
    /// pre-generated home anchors and (via `for_players`) the galaxy radius.
    pub max_players: u32,

    /// Speed of light, in sim-units per second. The single most important dial
    /// for the information model: smaller `c` = larger delays. Distances across
    /// the galaxy are tuned (with `galaxy_radius`) to produce delays in the
    /// seconds-to-minutes range. Consumed from M3 onward.
    pub c: f64,

    /// Base galaxy radius in sim units. Scales with player count so the dark
    /// space between homes stays proportional across 4–12 players (§4).
    pub galaxy_radius: f64,

    /// Fraction of `galaxy_radius` at which home anchors sit (a ring of bright
    /// spots between the hub and the rim).
    pub home_ring_frac: f64,

    /// Number of procedurally-placed star systems (M2).
    pub system_count: u32,

    /// Sensor detection radius (sim units) projected by each of a player's
    /// assets — their command center and every one of their ships. The player's
    /// sensor coverage is the union of these radii. Within coverage they detect
    /// dark raiders and read convoy cargo; outside it they are blind to raiders.
    pub sensor_range: f64,
}

impl SimConfig {
    /// A default galaxy sized for `player_count` players (galaxy radius scales
    /// with player count per §4 so inter-home distance stays proportional).
    pub fn for_players(seed: u64, player_count: u32) -> Self {
        let player_count = player_count.max(1);
        // Radius grows ~sqrt(players): area scales with player count so density
        // of homes stays roughly constant.
        let galaxy_radius = 4000.0 * (player_count as f64).sqrt();
        SimConfig {
            seed,
            max_players: player_count,
            // c chosen so crossing a 4-player galaxy (radius 8000) gives a
            // home→hub light-delay of ~16 s — dramatic but playable. All ship
            // speeds stay well below c (relativity is respected). Refined in M3.
            c: 300.0,
            galaxy_radius,
            home_ring_frac: 0.62,
            system_count: 12 + player_count * 4,
            // Local sensor bubbles (~28% of galaxy radius): coverage is islands
            // around your assets, so most of the dark between homes is blind to
            // raiders — the tension the model wants.
            sensor_range: 2200.0,
        }
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig::for_players(0xC0FFEE, 4)
    }
}

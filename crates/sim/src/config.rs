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

    /// Speed of light, in sim-units per second. The single most important dial
    /// for the information model: smaller `c` = larger delays. Distances across
    /// the galaxy are tuned (with `galaxy_radius`) to produce delays in the
    /// seconds-to-minutes range. Consumed from M3 onward.
    pub c: f64,

    /// Base galaxy radius in sim units. Scales with player count so the dark
    /// space between homes stays proportional across 4–12 players (§4).
    pub galaxy_radius: f64,

    /// Number of procedurally-placed star systems (M2).
    pub system_count: u32,
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
            // c chosen so crossing a 4-player galaxy (~radius 8000) takes tens
            // of seconds of light-delay — dramatic but playable. Refined in M3.
            c: 200.0,
            galaxy_radius,
            system_count: 12 + player_count * 4,
        }
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig::for_players(0xC0FFEE, 4)
    }
}

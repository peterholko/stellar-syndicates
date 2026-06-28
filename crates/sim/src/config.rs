//! Tunable constants for a SOLAR SYSTEM. Serialisable so the exact configuration
//! is captured in snapshots and shipped to the client for rendering.
//!
//! Spatial scale is the ASTRONOMICAL UNIT: positions are AU × [`AU`] sim-units.
//! The speed of light [`C`] is derived from the real AU light-crossing time and a
//! small [`TIME_COMPRESSION`], so light-delays are physically scaled — minutes
//! near the inner system, hours out at the Kuiper edge — which already suits the
//! async, command-lagged model (§5.1, §6).

use serde::{Deserialize, Serialize};

/// Simulation tick rate. The authoritative loop advances the world this many
/// times per real second; `dt = 1 / tick_hz`.
pub const TICK_HZ: u32 = 30;

/// Fixed timestep in seconds.
pub const DT: f64 = 1.0 / TICK_HZ as f64;

/// Sim-units per astronomical unit — the spatial scale. A body at `d` AU sits at
/// `d × AU` sim-units from the sun (origin).
pub const AU: f64 = 10_000.0;

/// Real seconds for light to cross 1 AU (≈ 8.317 light-minutes).
pub const REAL_SECONDS_PER_AU: f64 = 8.317 * 60.0;

/// How many× faster than real the game runs light/travel delays. Kept SMALL so
/// solar-system light-delays stay close to real (Earth-orbit ≈ light-minutes, the
/// Kuiper edge ≈ light-hours) — real delays already suit async play, so we don't
/// distort them much. Tunable.
pub const TIME_COMPRESSION: f64 = 3.0;

/// Speed of light in sim-units / sim-second, derived so a signal crosses 1 AU in
/// `REAL_SECONDS_PER_AU / TIME_COMPRESSION` sim-seconds (≈ 166 s at 3×, i.e. one
/// AU of light-delay ≈ 2.8 sim-minutes). ≈ 60.2 units/s.
pub const C: f64 = AU * TIME_COMPRESSION / REAL_SECONDS_PER_AU;

/// Light-delay (sim-seconds) over a distance of `au` astronomical units — for
/// docs/tests: `au × AU / C`.
pub fn light_delay_secs(au: f64) -> f64 {
    au * AU / C
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    /// Seed for all deterministic generation in this galaxy.
    pub seed: u64,

    /// Maximum players this galaxy is sized for. Determines the number of
    /// pre-generated home anchors and (via `for_players`) the galaxy radius.
    pub max_players: u32,

    /// Speed of light, in sim-units per second (= [`C`]). The single most
    /// important dial for the information model: light-delay over a distance is
    /// `dist / c`. At AU scale this yields minutes near the inner system, hours at
    /// the Kuiper edge — the physically-scaled fog/command-lag (§6).
    pub c: f64,

    /// Solar-system radius in sim-units (the Kuiper edge + margin). Sets the view
    /// filter's history horizon and the client's bounding circle.
    pub galaxy_radius: f64,

    /// AU distance at which the player STARTING ASTEROIDS (mining stations) sit —
    /// spaced apart so players don't begin on top of each other.
    pub start_orbit_au: f64,

    /// Number of inner-belt asteroids (~2–6 AU, accessible, lower-value ore).
    pub inner_belt: u32,
    /// Number of outer/frontier-belt asteroids (~9–22 AU, the dangerous, richer
    /// frontier — out at the ~1-light-hour rim).
    pub outer_belt: u32,

    /// Sensor detection radius (sim units) projected by each of a player's
    /// assets — their command center and every one of their ships. The player's
    /// sensor coverage is the union of these radii. Within coverage they detect
    /// dark raiders and read convoy cargo; outside it they are blind to raiders.
    pub sensor_range: f64,
}

impl SimConfig {
    /// A default solar system sized for `player_count` players. The system extent
    /// is fixed (it doesn't grow with players — players just get spaced starting
    /// asteroids); `max_players` sets how many starting asteroids are generated.
    pub fn for_players(seed: u64, player_count: u32) -> Self {
        let player_count = player_count.max(1);
        SimConfig {
            seed,
            max_players: player_count,
            c: C,
            // ~24 AU: just past the frontier rim (~22 AU), leaving margin for the
            // horizon. Bodies are spread to FILL this disk evenly (see galaxy.rs) —
            // the map is a playable board, not a realistic cramped-core system.
            galaxy_radius: 24.0 * AU,
            // Starting mining-station asteroids at ~7 AU — in the accessible mid
            // zone between the inner belt and the frontier, spaced apart.
            start_orbit_au: 7.0,
            inner_belt: 10,
            outer_belt: 7,
            // ~0.45 AU local sensor bubble — coverage is islands around your
            // assets, so most of the dark between bodies is blind to raiders.
            sensor_range: 0.45 * AU,
        }
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig::for_players(0xC0FFEE, 3)
    }
}

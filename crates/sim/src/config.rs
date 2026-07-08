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

/// THE LIGHT-GAME INVARIANT (playtest-driven): light must comfortably outrun the
/// fastest ship, or intel and orders arrive uselessly stale and raiders feel
/// "faster than light." Every preset must satisfy
/// `c ≥ C_SPEED_RATIO × fastest_ship_speed()`.
///
/// Default **2.0** — "at least twice, maybe more." Raising it is trivial: bump
/// this one number and every preset is re-checked at construction
/// ([`SimConfig::for_players`] asserts it) and in the unit tests. The shipped
/// playtest `c` sits well ABOVE this floor (≈3.5× the fastest hull) — the floor
/// is the guardrail, not the target.
pub const C_SPEED_RATIO: f64 = 2.0;

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

    /// The target DURATION (seconds) two EQUAL reference forces take to grind to
    /// their retreat thresholds — the strategic timescale of a decisive battle.
    /// Drives the Lanchester damage rate ([`crate::combat::dmg_rate`] =
    /// `DMG_RATE_CALIBRATION / battle_target_secs`). PRESETS: **playtest ≈ 45 s**
    /// (30–60 s band — travel is quick in the 12-player galaxy) · **production
    /// ≈ 2700 s** (30–60 min — battles at the scale of light-delays and relief
    /// travel). Lopsided fights still end fast; a safety valve
    /// ([`crate::combat::MAX_BATTLE_MULT`]) caps a no-retreat grind. serde default
    /// keeps old snapshots loading.
    #[serde(default = "default_battle_target_secs")]
    pub battle_target_secs: f64,

    /// Sim-time (seconds) at which every EXOTIC system AWAKENS into a capturable
    /// NODE (§node) — the midgame catalyst. Chosen mid-campaign: long enough that
    /// homes are established and a few frontier claims have landed, short enough
    /// that the awakening reshapes the back half. Telegraphed from t=0 and again in
    /// the run-up window; the awakening itself is a single deterministic sim event
    /// announced galaxy-wide, light-delayed. serde default keeps old snapshots
    /// loading. PRESETS mirror `battle_target_secs`: playtest ≈ 180 s, production
    /// scaled up alongside the battle timescale.
    #[serde(default = "default_node_awakening_time")]
    pub node_awakening_time: f64,
}

/// The default battle timescale (the PLAYTEST preset) — see `battle_target_secs`.
fn default_battle_target_secs() -> f64 {
    45.0
}

/// The default node-awakening time (PLAYTEST preset) — see `node_awakening_time`.
fn default_node_awakening_time() -> f64 {
    180.0
}

impl SimConfig {
    /// A default galaxy sized for `player_count` players (galaxy radius scales
    /// with player count per §4 so inter-home distance stays proportional).
    pub fn for_players(seed: u64, player_count: u32) -> Self {
        let player_count = player_count.max(1);
        // Radius grows ~sqrt(players): area scales with player count so density
        // of homes stays roughly constant.
        let galaxy_radius = 4000.0 * (player_count as f64).sqrt();
        let cfg = SimConfig {
            seed,
            max_players: player_count,
            // c chosen so crossing a 4-player galaxy (radius 8000) gives a
            // home→hub light-delay of ~12 s — dramatic but playable. Raised from
            // 300 → 400 (playtest: raiders felt "faster than light," intel/orders
            // uselessly stale) so light comfortably outruns every hull: at the
            // fastest ship (scout, 115) the ratio is 3.48×, well above the 2.0
            // floor ([`C_SPEED_RATIO`]). Ship trip-times are unchanged (they don't
            // depend on c); only information delays shrink ~25%, freshening intel.
            c: 400.0,
            galaxy_radius,
            home_ring_frac: 0.62,
            system_count: 12 + player_count * 4,
            // Local sensor bubbles (~28% of galaxy radius): coverage is islands
            // around your assets, so most of the dark between homes is blind to
            // raiders — the tension the model wants.
            sensor_range: 2200.0,
            // PLAYTEST preset: equal squadrons grind for ~45 s (production ships
            // ~2700 s / 45 min — battles at the scale of light-delays + relief).
            battle_target_secs: default_battle_target_secs(),
            // Exotic nodes awaken mid-campaign (§node) — telegraphed from t=0.
            node_awakening_time: default_node_awakening_time(),
        };
        // Structural guardrail: a future speed-table edit (or a mistuned c)
        // that let a ship approach light would silently break the whole
        // information game. Catch it at construction in debug/test builds.
        debug_assert!(
            cfg.satisfies_light_invariant(),
            "light-game invariant violated: c={} < {}× fastest ship speed {} \
             (ratio {:.2}). Raise c or trim the speed table (see C_SPEED_RATIO).",
            cfg.c,
            C_SPEED_RATIO,
            crate::ship::fastest_ship_speed(),
            cfg.light_ratio(),
        );
        cfg
    }

    /// How many times faster light travels than the fastest ship — the headroom
    /// the whole information game rests on. Must stay ≥ [`C_SPEED_RATIO`].
    pub fn light_ratio(&self) -> f64 {
        self.c / crate::ship::fastest_ship_speed()
    }

    /// Whether this config honours [the light-game invariant](C_SPEED_RATIO):
    /// `c ≥ C_SPEED_RATIO × fastest_ship_speed()`. Enforced at construction and
    /// asserted for every preset in the tests.
    pub fn satisfies_light_invariant(&self) -> bool {
        self.light_ratio() >= C_SPEED_RATIO
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig::for_players(0xC0FFEE, 4)
    }
}

#[cfg(test)]
mod light_invariant_tests {
    use super::*;

    /// Both shipped presets — the PLAYTEST galaxy (default `battle_target_secs`)
    /// and a PRODUCTION-tuned one (long battles) share the same `c` and speed
    /// table, so both must clear the light-game floor with margin. This test is
    /// the structural lock the prompt asks for: a future speed bump that outran
    /// light would fail here (and the `for_players` debug_assert) before shipping.
    #[test]
    fn light_invariant_holds_for_both_presets() {
        let fastest = crate::ship::fastest_ship_speed();

        // PLAYTEST preset (default).
        let playtest = SimConfig::for_players(1, 12);
        assert!(
            playtest.satisfies_light_invariant(),
            "playtest ratio {:.3} < {}",
            playtest.light_ratio(),
            C_SPEED_RATIO
        );

        // PRODUCTION preset: same c/speeds, long battles.
        let mut production = SimConfig::for_players(1, 12);
        production.battle_target_secs = 2700.0;
        assert!(
            production.satisfies_light_invariant(),
            "production ratio {:.3} < {}",
            production.light_ratio(),
            C_SPEED_RATIO
        );

        // Sanity: the shipped c really does clear the floor WITH margin, not by a
        // hair — light outruns the fastest hull (scout) by well over 2×.
        assert!(
            playtest.c >= C_SPEED_RATIO * fastest,
            "c {} below floor {}",
            playtest.c,
            C_SPEED_RATIO * fastest
        );
        assert!(
            playtest.light_ratio() > 3.0,
            "expected comfortable (>3×) margin, got {:.3}",
            playtest.light_ratio()
        );
    }
}

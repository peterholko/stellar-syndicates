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
/// fastest cruising ship, or intel and orders arrive uselessly stale. Every
/// preset must satisfy
/// `c ≥ C_SPEED_RATIO × fastest_ship_speed()`.
///
/// This is deliberately a cruise-speed guardrail. Chained jump-drive skips can
/// average about 5,000 su/s and outrun 2,000 su/s warp light; the information
/// model remains honest because each event still arrives on its own wavefront,
/// and the view's arrival heap may therefore serve those wavefronts out of
/// emission order.
///
/// Default **2.0** — "at least twice, maybe more." Raising it is trivial: bump
/// this one number and every preset is re-checked at construction
/// ([`SimConfig::for_players`] asserts it) and in the unit tests. The shipped
/// playtest `c` sits well ABOVE this floor (≈3.5× the fastest hull) — the floor
/// is the guardrail, not the target.
pub const C_SPEED_RATIO: f64 = 2.0;

/// Galaxy-size multiplier. Kept independent of drive constants so tuning warp
/// speed does not silently resize a generated chart.
pub const GALAXY_SCALE: f64 = 50.0;

/// §hub-district: the galaxy of one wormhole hub and its 20 homes, in sim
/// units: the four-player size, which every larger galaxy keeps.
pub const ONE_HUB_RADIUS: f64 = 400_000.0;

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

    /// Fraction of `galaxy_radius` at which home anchors sit: the middle of
    /// the hub district's band ([`crate::galaxy::HOME_RING_SU`]). The default
    /// keeps the physical home ring stable even when player count changes the
    /// radius.
    pub home_ring_frac: f64,

    /// Number of full frontier systems, excluding homes and exploration stars.
    /// This is the established colony/habitable-world budget, not total stars.
    pub system_count: u32,

    /// Additional sparse, non-habitable exploration systems. Generated AFTER
    /// the established galaxy so homes and habitable worlds never get rerolled.
    /// Old saves default to zero; loading a galaxy never inserts new geography.
    #[serde(default)]
    pub exploration_system_count: u32,

    /// Sensor detection radius (sim units) projected by each of a player's
    /// assets — their command center and every one of their ships. The player's
    /// sensor coverage is the union of these radii. Within coverage they detect
    /// dark raiders and read convoy cargo; outside it they are blind to raiders.
    pub sensor_range: f64,

    /// The target DURATION (seconds) two EQUAL reference forces take to grind to
    /// their retreat thresholds — the strategic timescale of a decisive battle.
    /// §tactical: drives the engine's step CADENCE
    /// ([`crate::tactical::tac_step_ticks`] — short playtest battles step at
    /// 2 Hz so tactics stay visible, production at 1 Hz), with the per-step
    /// to-hit/damage calibration ([`crate::tactical::HIT_DMG_CAL`]) tuned so
    /// equal reference forces resolve near this target. PRESETS: **playtest
    /// ≈ 45 s** (30–60 s band — travel is quick in the 12-player galaxy) ·
    /// **production ≈ 2700 s** (30–60 min — battles at the scale of
    /// light-delays and relief travel). Lopsided fights still end fast; a
    /// safety valve ([`crate::combat::MAX_BATTLE_MULT`]) caps a no-retreat
    /// grind. serde default keeps old snapshots loading.
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

    // §async-clocks (R4, Oct 22 2026): territory runs on clocks a player who
    // checks in a few times a day can answer. Battles stay on
    // `battle_target_secs`; only the stakes around them are long. Tests that
    // exercise the mechanics set short values. serde defaults let a running
    // galaxy pick these up on restart.
    /// How long an unbroken, defense-suppressed blockade must hold before a
    /// colony can be captured (before garrison and battleship adjustments).
    #[serde(default = "default_siege_capture_secs")]
    pub siege_capture_secs: f64,
    /// How long after a war declaration reaches its target the war goes live.
    #[serde(default = "default_war_notice_secs")]
    pub war_notice_secs: f64,
    /// How long neutral-space aggression gives its victim a right of reprisal.
    #[serde(default = "default_reprisal_secs")]
    pub reprisal_secs: f64,
}

fn default_siege_capture_secs() -> f64 {
    6.0 * 3600.0
}

fn default_war_notice_secs() -> f64 {
    crate::diplomacy::WAR_NOTICE_GRACE_S
}

fn default_reprisal_secs() -> f64 {
    crate::diplomacy::REPRISAL_S
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
        // Radius grows ~sqrt(players) up to four, then holds: §hub-district,
        // up to 20 players share one wormhole hub's 400,000 su galaxy, its
        // frontier denser rather than farther. (More hubs will revisit this.)
        // Preserve the established chart scale independently of the movement
        // and information-speed tunables.
        let galaxy_radius = (4000.0 * (player_count as f64).sqrt() * crate::config::GALAXY_SCALE).min(ONE_HUB_RADIUS);
        let cfg = SimConfig {
            seed,
            max_players: player_count,
            // Base c becomes 2,000 su/s straight warp light after WARP_FACTOR.
            // §hub-district: this prices home→hub at about 62 s (50–75 s across
            // the band) and the longest hub→rim reports near 200 s. At the
            // fastest warp hull the ratio remains 3.48×, above [`C_SPEED_RATIO`].
            c: 400.0,
            galaxy_radius,
            // §hub-district: homes 125,000 su from the hub, whatever the
            // player count (`galaxy::generate_home_slots` lays out the band).
            home_ring_frac: crate::galaxy::HOME_RING_SU / galaxy_radius,
            system_count: 12 + player_count * 4,
            // Double the chart's stars, not its habitable planets: 32 → 64 in
            // the default four-player galaxy (28 frontier + 4 home + 32 sparse).
            // Each home's six-system neighbourhood (galaxy::HOME_NEIGHBOURS,
            // a third habitable) brings it to 88, and §hub-district's stars
            // between the hub and the homes to 100.
            exploration_system_count: 12 + player_count * 5 + crate::galaxy::HUB_GAP_STARS,
            // §sensors (Oct 16 2026): a Command Center's own sensors reach
            // 6,000 su — one Raider at full throttle; a holding one only inside
            // the home's 1,300 su near zone. Seeing farther takes Sensor Arrays
            // or sensor fits on ships (Raider pickets carry this same bubble;
            // Convoys a 0.25× traffic set). Declared transponders are heard out
            // to `detection::NOISE_HORIZON_SU` instead.
            // Expressed through GALAXY_SCALE to preserve the chart-unit convention;
            // system-scale constants (docking, hyperlimit, claim) do not scale.
            sensor_range: 120.0 * crate::config::GALAXY_SCALE,
            // PLAYTEST preset: equal squadrons grind for ~45 s (production ships
            // ~2700 s / 45 min — battles at the scale of light-delays + relief).
            battle_target_secs: default_battle_target_secs(),
            // Exotic nodes awaken mid-campaign (§node) — telegraphed from t=0.
            node_awakening_time: default_node_awakening_time(),
            siege_capture_secs: default_siege_capture_secs(),
            war_notice_secs: default_war_notice_secs(),
            reprisal_secs: default_reprisal_secs(),
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
    /// §async-clocks (R4): territory runs on hours, battles stay short.
    #[test]
    fn async_clocks_default_to_hours() {
        let cfg = SimConfig::for_players(1, 12);
        assert_eq!(cfg.siege_capture_secs, 6.0 * 3600.0);
        assert_eq!(cfg.war_notice_secs, 2.0 * 3600.0);
        assert_eq!(cfg.reprisal_secs, 12.0 * 3600.0);
        assert_eq!(cfg.battle_target_secs, 45.0, "battles keep their playtest scale");
        // A galaxy saved before these clocks existed loads with them.
        let mut old = serde_json::to_value(&cfg).unwrap();
        for key in ["siege_capture_secs", "war_notice_secs", "reprisal_secs"] {
            old.as_object_mut().unwrap().remove(key);
        }
        let loaded: SimConfig = serde_json::from_value(old).unwrap();
        assert_eq!((loaded.siege_capture_secs, loaded.war_notice_secs, loaded.reprisal_secs),
            (6.0 * 3600.0, 2.0 * 3600.0, 12.0 * 3600.0));
    }

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
        // hair — light outruns the fastest rated hull (scout) by well over 3×;
        // §reactors-drives: the drive ladder spends that headroom down to the
        // 2× floor (a Scout on a Maelstrom).
        assert!(
            playtest.c >= C_SPEED_RATIO * fastest,
            "c {} below floor {}",
            playtest.c,
            C_SPEED_RATIO * fastest
        );
        assert!(
            playtest.c / crate::ship::fastest_rated_speed() > 3.0,
            "expected comfortable (>3×) margin on the rated table, got {:.3}",
            playtest.c / crate::ship::fastest_rated_speed()
        );
    }

    /// §hub-district: every home within the band around the hub, and no two
    /// closer than the spacing, for every galaxy one hub holds.
    #[test]
    fn homes_ring_the_hub_inside_the_band_and_keep_their_spacing() {
        use crate::galaxy::{HOME_BAND_SU, HOME_RING_JITTER_SU, HOME_RING_SU, HOME_SPACING_SU};
        for players in 1..=20 {
            for seed in [1, 7, 42, 0xC0FFEE] {
                let cfg = SimConfig::for_players(seed, players);
                let mut rng = crate::rng::Rng::new(seed);
                let slots = crate::galaxy::generate_home_slots(&mut rng, cfg.galaxy_radius, cfg.home_ring_frac, players);
                assert_eq!(slots.len(), players as usize);
                let one_ring = players <= 12;
                for slot in &slots {
                    let out = slot.pos.length();
                    let band = if one_ring { HOME_RING_JITTER_SU } else { HOME_BAND_SU / 2.0 };
                    assert!((out - HOME_RING_SU).abs() <= band + 1e-6, "{players} players: a home {out:.0} su out");
                }
                for (i, a) in slots.iter().enumerate() {
                    for b in &slots[i + 1..] {
                        let d = a.pos.distance(b.pos);
                        assert!(d >= HOME_SPACING_SU - 1e-6, "{players} players, seed {seed}: homes {d:.0} su apart");
                    }
                }
            }
        }
    }
}

//! EXOTIC NODE AWAKENING (§node) — the midgame catalyst.
//!
//! Some star systems are EXOTIC (black holes, magnetars, pulsars, binaries). The
//! CLIENT already paints them as special from a deterministic hash of the system
//! id (`client/src/stars.ts`), a purely cosmetic layer. This module promotes that
//! same set into a MECHANICAL midgame event: at [`SimConfig::node_awakening_time`]
//! every exotic system AWAKENS into a **node** — a capturable strategic prize
//! granting ONE tactical bonus to whoever holds it.
//!
//! DETERMINISM / ONE SOURCE OF TRUTH. The exotic set MUST match the client's
//! visual exotics exactly, or a black-hole icon would grant no node (and vice
//! versa). So [`node_bonus_for`] REPLICATES the client's FNV-1a assignment bit for
//! bit: it hashes the system id's DECIMAL-STRING form (how `EntityId` serialises on
//! the wire — the very string the client hashes), applies the same rarity gate and
//! pool index, and maps each exotic star type to a bonus. Change either side and
//! this pairing breaks — they are one algorithm expressed twice, kept in lockstep
//! by the parity test in the sim tests and the shared constants below.
//!
//! The three bonuses each plug into ONE existing function so the fog stays honest:
//!   * Relay Anchor → `World::relay_factor` scales the command-delay leg in
//!     `schedule_for_owner` (the single command-delay choke point).
//!   * Veil → `World::veil_factor` scales `detection::signature` at the SAME two
//!     detection sites the pickets and the View already share.
//!   * Deep Scan → `World::deep_scan_covers` gates the composition-reveal ladder in
//!     the server View (bucket→exact) — no new leak, just an earlier reveal.
//!
//! Bonuses unlock TACTICS, never economy multipliers (anti-snowball). Holding one
//! costs a per-tick UPKEEP MIX drawn from the node's LOCAL stockpile; starve it and
//! the bonus SUSPENDS (Habitat idiom — nothing destroyed, recovers when fed). A
//! corp benefits from at most [`NODES_PER_CORP`] nodes at once (deterministic pick
//! by id). Capture/flip is ANNOUNCED galaxy-wide, light-delayed (Exposure).

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::EntityId;

// ── Tunables (MECHANICS are the deliverable; these are the dials) ───────────────

/// Radius (sim units) of a node's REGION — the area within which its bonus applies.
/// Sized a touch under a home→mid-ring hop so a node commands a neighbourhood, not
/// the whole galaxy (bonus is local leverage, not a blanket buff). Tunable.
pub const NODE_REGION_RADIUS: f64 = 1800.0;

/// How many nodes a single corp may draw a bonus from AT ONCE. Caps stacking so a
/// runaway can't monopolise every tactical edge; excess held nodes still cost
/// upkeep and deny rivals, but grant nothing extra. Deterministic pick (lowest
/// system id first). Tunable (1–2).
pub const NODES_PER_CORP: usize = 1;

/// Relay Anchor — the command-delay MULTIPLIER for orders to targets inside the
/// region of a fed black-hole node the issuer holds. `0.5` ⇒ orders (and their
/// echoes) land in half the light-time within that neighbourhood.
pub const RELAY_DELAY_MULT: f64 = 0.5;

/// Veil — the signature MULTIPLIER for the holder's DARK fleets inside a fed
/// magnetar node's region. `0.5` halves how far they're detected (plugs the
/// existing `cloak_mult` scope regionally). Broadcasters are unaffected (they
/// announce themselves; the Veil only quiets the quiet).
pub const VEIL_SIGNATURE_MULT: f64 = 0.5;

/// The per-second UPKEEP MIX an AWAKENED, OWNED node draws from its OWN system's
/// stockpile. All-or-nothing per tick (like Habitat/Garrison): the whole mix is
/// affordable or the node goes UNFED and its bonus suspends. A modest mix of the
/// two refined staples — enough that a node is a commitment, not free real estate.
pub const NODE_UPKEEP_PER_SEC: &[(Commodity, f64)] =
    &[(Commodity::Provisions, 4.0), (Commodity::Fuel, 2.0)];

// ── The exotic → bonus mapping (client-parity constants) ────────────────────────

/// Fraction of systems whose star is EXOTIC — MUST equal `EXOTIC_FRACTION` in
/// `client/src/stars.ts`. A system is a node iff its roll lands under this.
pub const EXOTIC_FRACTION: f64 = 0.16;

/// The bonus a node grants. Exactly one per node, fixed by its exotic star type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeBonus {
    /// Black hole — halves command delay to targets in the region (tempo).
    RelayAnchor,
    /// Magnetar — friendly dark fleets in the region run quieter (concealment).
    Veil,
    /// Pulsar / binary — the holder's sensors resolve EXACT composition on any
    /// already-visible fleet in the region (bucket→exact certainty).
    DeepScan,
}

impl NodeBonus {
    /// Stable machine slug (shipped to the client for its label/tooltip table).
    pub fn slug(self) -> &'static str {
        match self {
            NodeBonus::RelayAnchor => "relay_anchor",
            NodeBonus::Veil => "veil",
            NodeBonus::DeepScan => "deep_scan",
        }
    }

    /// Human title (the node's headline in reports/panels).
    pub fn title(self) -> &'static str {
        match self {
            NodeBonus::RelayAnchor => "Relay Anchor",
            NodeBonus::Veil => "Veil",
            NodeBonus::DeepScan => "Deep Scan",
        }
    }
}

/// A dormant-then-awakened NODE, keyed in [`crate::world::World::nodes`] by its host
/// system id. The HOLDER is the host system's `owner` (read live — no duplicated
/// ownership), so capture/flip needs no node-specific plumbing. `awakened` latches
/// once at [`SimConfig::node_awakening_time`]; `fed` is recomputed each tick from
/// upkeep (like `StarSystem::habitat_fed`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Which tactical bonus this node grants (fixed at seeding by star type).
    pub bonus: NodeBonus,
    /// Has this node activated yet? False until the awakening time; latches true.
    #[serde(default)]
    pub awakened: bool,
    /// Is the node's upkeep currently met? A fed node's bonus is LIVE; an unfed
    /// one SUSPENDS it (nothing destroyed). Presumed fed until first shortfall.
    #[serde(default = "default_true")]
    pub fed: bool,
}

fn default_true() -> bool {
    true
}

impl Node {
    /// A freshly-seeded, still-dormant node of the given bonus.
    pub fn dormant(bonus: NodeBonus) -> Self {
        Node { bonus, awakened: false, fed: true }
    }

    /// A node grants its bonus only when it has AWAKENED and is currently FED.
    pub fn active(&self) -> bool {
        self.awakened && self.fed
    }
}

/// Deterministic 32-bit FNV-1a of a byte string — the EXACT algorithm in
/// `client/src/stars.ts::hashId`. The id is hashed as its decimal-string form
/// (ASCII digits only, so `bytes()` equals JS `charCodeAt`).
fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// The bonus for a system id, or `None` if the system is not exotic — the SIM twin
/// of `client/src/stars.ts::starTypeFor`. Replicates the roll (`h % 100000 /
/// 100000 < EXOTIC_FRACTION`) and the pool index (`(h >> 17) % 4`) over the client's
/// EXOTIC pool order `[neutron_star, binary_star, black_hole, magnetar]`, then maps
/// each star type to its bonus. Keep in lockstep with the client (parity test).
pub fn node_bonus_for(id: EntityId) -> Option<NodeBonus> {
    let h = fnv1a(&id.0.to_string());
    let roll = (h % 100_000) as f64 / 100_000.0;
    if roll >= EXOTIC_FRACTION {
        return None; // realistic (common) star — no node
    }
    // EXOTIC pool index, MATCHING the client's `EXOTIC.filter` order:
    //   0 neutron_star · 1 binary_star · 2 black_hole · 3 magnetar
    Some(match (h >> 17) % 4 {
        0 => NodeBonus::DeepScan,    // neutron_star / pulsar → sensor certainty
        1 => NodeBonus::DeepScan,    // binary_star           → sensor certainty
        2 => NodeBonus::RelayAnchor, // black_hole            → command tempo
        _ => NodeBonus::Veil,        // magnetar              → concealment
    })
}

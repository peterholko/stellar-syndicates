//! Cargo carried by convoys. A convoy *broadcasts* its identity and position
//! (the Galactic Convention, §6) but NOT its cargo — cargo contents are only
//! revealed to a player whose sensors are within range of the convoy (the
//! two-tier information model). For now cargo is demo content; in the economy
//! milestone (§9) it becomes the real traded goods.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Commodity {
    Fuel,
    Ore,
    Alloys,
    Provisions,
    Volatiles,
}

impl Commodity {
    pub const ALL: [Commodity; 5] = [
        Commodity::Fuel,
        Commodity::Ore,
        Commodity::Alloys,
        Commodity::Provisions,
        Commodity::Volatiles,
    ];
}

/// A convoy's manifest: what it is hauling and how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cargo {
    pub commodity: Commodity,
    pub units: u32,
}

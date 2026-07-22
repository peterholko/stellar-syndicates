//! Stable identifiers for simulation entities.
//!
//! Ids are plain integers so the pure core stays deterministic and serialises
//! compactly. `PlayerId` is assigned by the server (a stable hash of the
//! player's name/token) and handed to the sim via commands, so a reconnecting
//! player resolves to the same corporation (needed for M6 reconnect).

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Identifies a player's corporation. Assigned outside the sim and passed in
/// via [`crate::command::Command`].
///
/// The inner value is a full 64-bit hash, which exceeds JavaScript's safe
/// integer range (2^53). To stay precise across the wire and in snapshots,
/// `PlayerId` (de)serialises as a **decimal string**, not a JSON number. (The
/// JS client must therefore treat it as a string.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u64);

impl PlayerId {
    /// The neutral PIRATE faction (§pirates) — a reserved SENTINEL that owns
    /// pirate raider packs but is NOT a real [`crate::world::Corporation`] in
    /// `World.players`. Hostile to all (never in any syndicate; `are_allied`
    /// returns false for it), driven solely by `World::pirate_ai`. Its value is a
    /// distinctive high tag far from any name hash; the server's
    /// `player_id_from_name` guards against ever colliding with it.
    pub const PIRATE: PlayerId = PlayerId(0x5049_5241_5445_0000); // "PIRATE\0\0"

    /// The neutral TERRAN CHARTER AUTHORITY faction (§TCA) — a reserved SENTINEL
    /// that owns the scheduled [`crate::ship::ShipKind::Freighter`] fleets but is
    /// NOT a real [`crate::world::Corporation`] in `World.players`. It holds no
    /// territory, never appears in rankings or valuation, and is hostile to none
    /// (its hulls are just physical objects in a hostile world). Mirrors
    /// [`Self::PIRATE`]: a distinctive high tag far from any name hash; the
    /// server's `player_id_from_name` guards against ever colliding with it.
    pub const TCA: PlayerId = PlayerId(0x5443_4100_0000_0000); // "TCA\0\0\0\0\0"

    /// Whether this id is the neutral PIRATE faction.
    pub fn is_pirate(self) -> bool {
        self.0 == Self::PIRATE.0
    }

    /// Whether this id is the neutral Terran Charter Authority faction (§TCA).
    pub fn is_tca(self) -> bool {
        self.0 == Self::TCA.0
    }

    /// Whether this id is any NEUTRAL sentinel faction (PIRATE or TCA) rather than
    /// a real player corporation — the common test for "not a ranked/valued corp".
    pub fn is_sentinel(self) -> bool {
        self.is_pirate() || self.is_tca()
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P{:016x}", self.0)
    }
}

impl Serialize for PlayerId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for PlayerId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl de::Visitor<'_> for V {
            type Value = PlayerId;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a u64 as a decimal string or number")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<PlayerId, E> {
                v.parse::<u64>().map(PlayerId).map_err(de::Error::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<PlayerId, E> {
                Ok(PlayerId(v))
            }
        }
        d.deserialize_any(V)
    }
}

/// Identifies any spatial entity (ship, convoy, system, anchor) in the world.
/// Allocated deterministically by the [`crate::world::World`] from a counter.
///
/// Like [`PlayerId`], it (de)serialises as a **decimal string** so that ids
/// never lose precision in the JS client, keeping the whole id space uniform on
/// the wire (counters realistically stay small, but uniformity removes a latent
/// footgun once entities start streaming to the client in M2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub u64);

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "E{}", self.0)
    }
}

impl Serialize for EntityId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl de::Visitor<'_> for V {
            type Value = EntityId;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a u64 as a decimal string or number")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<EntityId, E> {
                v.parse::<u64>().map(EntityId).map_err(de::Error::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<EntityId, E> {
                Ok(EntityId(v))
            }
        }
        d.deserialize_any(V)
    }
}

/// Identifies a [`crate::syndicate::Syndicate`] (an alliance). Allocated
/// deterministically by the [`crate::world::World`] from its own counter (kept
/// separate from the entity counter). Like the other ids it (de)serialises as a
/// **decimal string** so the JS client never loses precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyndicateId(pub u64);

impl std::fmt::Display for SyndicateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "S{}", self.0)
    }
}

impl Serialize for SyndicateId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for SyndicateId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl de::Visitor<'_> for V {
            type Value = SyndicateId;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a u64 as a decimal string or number")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<SyndicateId, E> {
                v.parse::<u64>().map(SyndicateId).map_err(de::Error::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<SyndicateId, E> {
                Ok(SyndicateId(v))
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_id_serializes_as_decimal_string() {
        // A value beyond JS's safe integer range (2^53) must survive as a
        // string so the browser client doesn't silently lose precision.
        let pid = PlayerId(14_913_266_949_370_903_327);
        let json = serde_json::to_string(&pid).unwrap();
        assert_eq!(json, "\"14913266949370903327\"");
    }

    #[test]
    fn player_id_round_trips_at_u64_max() {
        let original = PlayerId(u64::MAX);
        let json = serde_json::to_string(&original).unwrap();
        let restored: PlayerId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn player_id_accepts_numeric_form_too() {
        // Lenient deserialisation: a bare number still parses (back-compat).
        let restored: PlayerId = serde_json::from_str("42").unwrap();
        assert_eq!(restored, PlayerId(42));
    }
}


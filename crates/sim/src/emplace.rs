//! §emplacements: STRUCTURES YOU PLACE IN OPEN SPACE, rather than in a system's
//! build slots.
//!
//! Deep-space sensors are player-built pickets: they shorten the observation
//! leg without modifying the straight warp-light report home.

use serde::{Deserialize, Serialize};

use crate::PlayerId;
use crate::math::Vec2;

/// What an emplacement is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmplacementKind {
    /// A stationary sensor picket. Watches like a ship's sensors would and sends
    /// what it sees home at warp — so it shortens the *observation* leg without
    /// modifying the transmission medium.
    DeepSpaceSensor,
}

impl EmplacementKind {
    pub const ALL: [EmplacementKind; 1] = [EmplacementKind::DeepSpaceSensor];

    pub fn label(self) -> &'static str {
        "Deep Space Sensor"
    }

    /// The bubble a deep space sensor projects.
    pub fn sensor_range(self) -> f64 {
        DEEP_SPACE_SENSOR_RANGE
    }
}

/// How far a deep space sensor sees. Deliberately shorter than a system's own
/// array: a picket buys you REACH, not better eyes, and stringing several out is
/// meant to cost more than upgrading one.
pub const DEEP_SPACE_SENSOR_RANGE: f64 = 60_000.0;

/// One placed structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Emplacement {
    pub id: crate::EntityId,
    pub owner: PlayerId,
    pub kind: EmplacementKind,
    pub pos: Vec2,
}

/// Why a proposed site was rejected — the client shows this while the player is
/// choosing where to put one, so the rule is discoverable rather than a silent
/// refusal after the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteError {
    /// Something of yours is already here.
    TooClose,
}

/// How close a builder must hold to its site for the work to run — a worksite,
/// not a rendezvous, so it is tight.
pub const CONSTRUCT_RADIUS: f64 = 600.0;

/// §emplacements: seconds a combatant must hold station on a RIVAL structure to
/// tear it down. Wrecking is easier than raising, but the hold is long enough
/// that the victim's picket, or the news itself,
/// has a chance to matter. The same `CONSTRUCT_RADIUS` bounds the work.
pub const DEMOLISH_SECONDS: f64 = 20.0;

/// How close two emplacements may be. Stops a player stacking a dozen sensors
/// on one spot for free redundancy.
pub const MIN_SPACING: f64 = 12_000.0;

/// Is `pos` a legal site for `kind`, given the network and what is already out
/// there? Pure, so the client can ask the same question the server will answer.
pub fn site_check(
    _kind: EmplacementKind,
    pos: Vec2,
    existing: &[Emplacement],
) -> Result<(), SiteError> {
    if existing.iter().any(|e| e.pos.distance(pos) < MIN_SPACING) {
        return Err(SiteError::TooClose);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A SENSOR GOES ANYWHERE. The frontier is exactly where a picket earns its
    /// keep, so open-space siting is the point of the structure.
    #[test]
    fn a_deep_space_sensor_goes_anywhere() {
        let nowhere = Vec2::new(2_000_000.0, 0.0);
        assert_eq!(
            site_check(EmplacementKind::DeepSpaceSensor, nowhere, &[]),
            Ok(())
        );
    }

    /// Not on top of each other: stacking would be free redundancy, not a network.
    #[test]
    fn emplacements_keep_their_distance() {
        let p = Vec2::new(10_000.0, 0.0);
        let existing = vec![Emplacement {
            id: crate::EntityId(1),
            owner: crate::PlayerId(1),
            kind: EmplacementKind::DeepSpaceSensor,
            pos: p,
        }];
        assert_eq!(
            site_check(EmplacementKind::DeepSpaceSensor, p, &existing),
            Err(SiteError::TooClose),
        );
        let far = p + Vec2::new(MIN_SPACING * 2.0, 0.0);
        assert_eq!(
            site_check(EmplacementKind::DeepSpaceSensor, far, &existing),
            Ok(())
        );
    }
}

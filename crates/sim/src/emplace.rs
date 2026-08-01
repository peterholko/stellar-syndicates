//! §emplacements: STRUCTURES YOU PLACE IN OPEN SPACE, rather than in a system's
//! build slots.
//!
//! Two kinds so far, and they exist for opposite halves of the same problem —
//! getting information home. A HYPERSPACE BUOY makes the trip faster; a DEEP
//! SPACE SENSOR makes the trip start closer to what you want to watch.
//!
//! Both are sited by the player on the galaxy map, both are owned, and both can
//! be taken away — which is the point. It is what turns the lane network from
//! geography you were handed into infrastructure you build, defend, and cut.

use serde::{Deserialize, Serialize};

use crate::math::Vec2;
use crate::PlayerId;

/// What an emplacement is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmplacementKind {
    /// Relays signals at hyperspace speed — but only BETWEEN TWO owned relays
    /// sharing a lane. The home system counts as one, so the first built buoy is
    /// useful when it shares home's lane; that access pair opens the connected
    /// lane graph through junctions. Home and buoys are the only entry/exit
    /// points for off-lane signals. Buildable only on a lane; see
    /// [`EmplacementKind::needs_a_lane`].
    HyperspaceBuoy,
    /// A stationary sensor picket. Watches like a ship's sensors would and sends
    /// what it sees home at warp — so it shortens the *observation* leg rather
    /// than the transmission, which is the half a buoy cannot help with.
    DeepSpaceSensor,
    /// §coupled: a listening post PARKED IN A LANE. A hull riding a lane is
    /// coupled to the medium, and coupling works both ways — the sensor hears
    /// the wake of rival traffic on its lane and reports home at lane speed, so
    /// a lane raider can no longer arrive ahead of the news of it PAST one of
    /// these. The counter-play is built in: drop to warp and go the slow, quiet
    /// way around, off the wire.
    HyperspaceSensor,
}

impl EmplacementKind {
    pub const ALL: [EmplacementKind; 3] = [
        EmplacementKind::HyperspaceBuoy,
        EmplacementKind::DeepSpaceSensor,
        EmplacementKind::HyperspaceSensor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EmplacementKind::HyperspaceBuoy => "Hyperspace Buoy",
            EmplacementKind::DeepSpaceSensor => "Deep Space Sensor",
            EmplacementKind::HyperspaceSensor => "Hyperspace Sensor",
        }
    }

    /// A buoy relays ALONG a lane, so it has to be on one. A sensor watches open
    /// space and may be put anywhere — the frontier is exactly where it earns
    /// its keep.
    pub fn needs_a_lane(self) -> bool {
        matches!(self, EmplacementKind::HyperspaceBuoy | EmplacementKind::HyperspaceSensor)
    }

    /// The bubble a deep space sensor projects. Zero for a buoy, which relays
    /// but does not watch.
    pub fn sensor_range(self) -> f64 {
        match self {
            EmplacementKind::HyperspaceBuoy => 0.0,
            EmplacementKind::DeepSpaceSensor => DEEP_SPACE_SENSOR_RANGE,
            // A lane listener hears the MEDIUM, not open space.
            EmplacementKind::HyperspaceSensor => 0.0,
        }
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
    /// A buoy has to sit in a lane to relay along it.
    NotOnALane,
    /// Something of yours is already here.
    TooClose,
}

/// §coupled: how far along its lane a hyperspace sensor HEARS, in arc length,
/// either way. A wake is noise, not a directed transmission — it attenuates —
/// so one post covers a corridor, not the whole route, and siting the approach
/// lanes stays a real decision.
pub const LANE_LISTEN_RANGE: f64 = 120_000.0;

/// How close a builder must hold to its site for the work to run — a worksite,
/// not a rendezvous, so it is tight.
pub const CONSTRUCT_RADIUS: f64 = 600.0;

/// §emplacements: seconds a combatant must hold station on a RIVAL structure to
/// tear it down. Shorter than any build (a buoy takes 45s) — wrecking is easier
/// than raising — but long enough that the victim's picket, or the news itself,
/// has a chance to matter. The same `CONSTRUCT_RADIUS` bounds the work.
pub const DEMOLISH_SECONDS: f64 = 20.0;

/// How close two emplacements may be. Stops a player stacking a dozen buoys on
/// one spot, which would be free redundancy rather than a network.
pub const MIN_SPACING: f64 = 12_000.0;

/// Is `pos` a legal site for `kind`, given the network and what is already out
/// there? Pure, so the client can ask the same question the server will answer.
pub fn site_check(
    kind: EmplacementKind,
    pos: Vec2,
    lanes: &crate::lane::LaneNetwork,
    existing: &[Emplacement],
) -> Result<(), SiteError> {
    if kind.needs_a_lane() && lanes.relay_at(pos).on.is_empty() {
        return Err(SiteError::NotOnALane);
    }
    if existing.iter().any(|e| e.pos.distance(pos) < MIN_SPACING) {
        return Err(SiteError::TooClose);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lane::{generate, LaneAnchor};

    fn net() -> (crate::lane::LaneNetwork, Vec<Vec2>) {
        let radius = 400_000.0;
        let mut rng = crate::rng::Rng::new(1);
        let anchors: Vec<LaneAnchor> = (0..32)
            .map(|_| {
                let t = rng.range(0.12 * 0.12, 0.96 * 0.96);
                LaneAnchor {
                    pos: Vec2::from_polar(rng.range(0.0, std::f64::consts::TAU), radius * t.sqrt()),
                }
            })
            .collect();
        let homes: Vec<Vec2> = (0..4)
            .map(|i| Vec2::from_polar(std::f64::consts::TAU * f64::from(i) / 4.0, radius * 0.62))
            .collect();
        let n = generate(1, Vec2::ZERO, &anchors, &homes, radius);
        (n, homes)
    }

    /// A BUOY HAS TO BE ON A LANE. It relays ALONG one, so open space is not a
    /// site — and the rule is checked here, where the client can ask it too, so
    /// the map can refuse the click rather than the server refusing the order.
    #[test]
    fn a_hyperspace_buoy_only_goes_on_a_lane() {
        let (lanes, _) = net();
        let on_lane = lanes.lanes[0].at(lanes.lanes[0].length() * 0.5);
        assert_eq!(site_check(EmplacementKind::HyperspaceBuoy, on_lane, &lanes, &[]), Ok(()));

        // Far outside the galaxy, where no ribbon reaches.
        let nowhere = Vec2::new(2_000_000.0, 0.0);
        assert_eq!(
            site_check(EmplacementKind::HyperspaceBuoy, nowhere, &lanes, &[]),
            Err(SiteError::NotOnALane),
        );
    }

    /// A SENSOR GOES ANYWHERE. The frontier is exactly where a picket earns its
    /// keep, so requiring a lane would forbid the only siting that matters.
    #[test]
    fn a_deep_space_sensor_goes_anywhere() {
        let (lanes, _) = net();
        let nowhere = Vec2::new(2_000_000.0, 0.0);
        assert_eq!(site_check(EmplacementKind::DeepSpaceSensor, nowhere, &lanes, &[]), Ok(()));
    }

    /// Not on top of each other: stacking would be free redundancy, not a network.
    #[test]
    fn emplacements_keep_their_distance() {
        let (lanes, _) = net();
        let p = lanes.lanes[0].at(lanes.lanes[0].length() * 0.5);
        let existing = vec![Emplacement {
            id: crate::EntityId(1),
            owner: crate::PlayerId(1),
            kind: EmplacementKind::HyperspaceBuoy,
            pos: p,
        }];
        assert_eq!(
            site_check(EmplacementKind::HyperspaceBuoy, p, &lanes, &existing),
            Err(SiteError::TooClose),
        );
        // A step further along the same lane is fine.
        let far = lanes.lanes[0].at(lanes.lanes[0].length() * 0.5 + MIN_SPACING * 2.0);
        assert_eq!(site_check(EmplacementKind::HyperspaceBuoy, far, &lanes, &existing), Ok(()));
    }
}

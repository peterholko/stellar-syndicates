//! §licences (docs/blueprint-licences.md, Peter Nov 8 2026): "Research is
//! basically unlocking the skill to build it but the Blueprint is the license
//! to build from the Authority." Every hull built at an Orbital Shipyard or a
//! Capital Slipway needs the Authority's licence as well as its research. A
//! licence is bought once, at the Hub, and kept for good. A syndicate at war
//! with the Authority needs none: independence is the endgame.
use serde::{Deserialize, Serialize};

use crate::build::StructureKind;
use crate::ship::ShipKind;

/// A licence's price at full standing, as a share of the hull's kit value at
/// base prices (Peter: "roughly 20% of the cost of the ship"). Tunable.
pub const LICENCE_SHARE: f64 = 0.20;

/// Does laying a keel of `kind` need the Authority's licence? Every hull from
/// an Orbital Shipyard (the Naval Drydock) or a Capital Slipway: the Large,
/// Heavy and Bulk Freighters and the Destroyer up. Nothing from the ground
/// Shipyard does.
pub fn needs_licence(kind: ShipKind) -> bool {
    matches!(crate::build::yard_for(kind).0, StructureKind::NavalDrydock | StructureKind::CapitalSlipway)
}

/// The capital licences (Dreadnought, Titan) the Authority sells only to a
/// Commissioned corporation.
pub fn commissioned_only(kind: ShipKind) -> bool {
    crate::build::yard_for(kind).0 == StructureKind::CapitalSlipway
}

/// Every hull that needs a licence, cheapest first.
pub fn licensed_hulls() -> Vec<ShipKind> {
    let mut hulls: Vec<ShipKind> = crate::ship::ALL_SHIP_KINDS.iter().copied().filter(|k| needs_licence(*k)).collect();
    hulls.sort_by(|a, b| crate::combat_balance::hull_value(*a).total_cmp(&crate::combat_balance::hull_value(*b)));
    hulls
}

/// What the Authority asks for `kind`'s licence at `standing`: a share of the
/// hull's kit value, scaled like its freight (`tca::tariff_mult`: ×0.85 /
/// ×0.70 / ×0.50 at Trusted / Preferred / Commissioned, up to ×3 as standing
/// falls toward Revoked).
pub fn price(kind: ShipKind, standing: f64) -> f64 {
    LICENCE_SHARE * crate::combat_balance::hull_value(kind) * crate::tca::tariff_mult(standing)
}

/// The Authority's answer for `kind`'s licence: its price at `standing`, or
/// why it won't sell. The one rule for the purchase (live, at the Hub) and the
/// view (as heard at home). `researched`: the hull's research is done.
pub fn offer(kind: ShipKind, standing: f64, researched: bool, held: bool, at_war: bool) -> Result<f64, LicenceRefusal> {
    if !needs_licence(kind) {
        return Err(LicenceRefusal::NotRequired);
    }
    if at_war {
        return Err(LicenceRefusal::AtWar);
    }
    if held {
        return Err(LicenceRefusal::AlreadyHeld);
    }
    if !researched {
        return Err(LicenceRefusal::NeedsResearch);
    }
    if crate::tca::charter_status(standing).exchange_closed() {
        return Err(LicenceRefusal::CharterClosed);
    }
    if commissioned_only(kind) && crate::tca::charter_grade(standing) < crate::tca::CharterGrade::Commissioned {
        return Err(LicenceRefusal::NeedsCommissioned);
    }
    Ok(price(kind, standing))
}

/// The price a hull's licence is shown at: today's, or a capital's at
/// Commissioned (the only grade that buys one).
pub fn shown_price(kind: ShipKind, standing: f64) -> f64 {
    let floor = if commissioned_only(kind) { crate::tca::TCA_COMMISSIONED_AT } else { f64::NEG_INFINITY };
    price(kind, standing.max(floor))
}

/// Why the Authority refused a licence. Owner-only; a refusal costs nothing
/// (the payment travels home).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum LicenceRefusal {
    /// The hull needs no licence (it is built at the ground Shipyard).
    NotRequired,
    /// The corporation already holds it.
    AlreadyHeld,
    /// Its research isn't done: the skill comes first.
    NeedsResearch,
    /// A capital licence, sold only at Commissioned.
    NeedsCommissioned,
    /// The charter is Revoked or Proscribed: the Exchange is closed to it.
    CharterClosed,
    /// At war with the Authority: it sells nothing, and nothing is needed.
    AtWar,
    /// The payment didn't cover the price.
    CantAfford { cost: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn licences_cover_every_orbital_and_slipway_hull_and_nothing_else() {
        let hulls = licensed_hulls();
        assert_eq!(hulls, vec![ShipKind::Destroyer, ShipKind::LargeFreighter, ShipKind::HeavyFreighter, ShipKind::Cruiser,
            ShipKind::BulkFreighter, ShipKind::Battleship, ShipKind::Dreadnought, ShipKind::Titan]);
        for kind in [ShipKind::Raider, ShipKind::Corvette, ShipKind::Scout, ShipKind::TinyFreighter, ShipKind::SmallFreighter,
            ShipKind::Convoy, ShipKind::MiningBeltStation, ShipKind::CombatFreighter, ShipKind::Colony] {
            assert!(!needs_licence(kind), "{kind:?} is a ground-Shipyard hull");
        }
        assert!(commissioned_only(ShipKind::Dreadnought) && commissioned_only(ShipKind::Titan));
        assert!(!commissioned_only(ShipKind::Battleship));
    }

    #[test]
    fn a_licence_is_a_fifth_of_the_hull_scaled_by_standing() {
        let hull = crate::combat_balance::hull_value(ShipKind::Destroyer);
        assert!((price(ShipKind::Destroyer, 100.0) - 0.2 * hull).abs() < 1e-6);
        assert!((price(ShipKind::Destroyer, 100.0) - 10_008.0).abs() < 100.0, "≈ 10,000 at full standing");
        assert!((price(ShipKind::Destroyer, 175.0) / price(ShipKind::Destroyer, 100.0) - 0.50).abs() < 1e-9);
        assert!((price(ShipKind::Destroyer, 150.0) / price(ShipKind::Destroyer, 100.0) - 0.70).abs() < 1e-9);
        assert!((price(ShipKind::Destroyer, 125.0) / price(ShipKind::Destroyer, 100.0) - 0.85).abs() < 1e-9);
        assert!((price(ShipKind::Destroyer, 60.0) / price(ShipKind::Destroyer, 100.0) - 2.0).abs() < 1e-9);
        assert!(price(ShipKind::Destroyer, 21.0) > 2.9 * price(ShipKind::Destroyer, 100.0));
        assert!((price(ShipKind::Titan, 175.0) - 13_047_065.0).abs() < 200_000.0, "≈ 13M at Commissioned");
    }
}

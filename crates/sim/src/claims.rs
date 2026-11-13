//! §claims (Peter, Nov 10 2026: "I want the player to have to purchase a claim
//! from the Authority for new colonies"; docs/scarcity-staples-plan.md step
//! 1.7). A Colony Ship founds a colony only on a system its corporation holds
//! the Authority's claim to. A claim is bought at the Hub for one system, held
//! by one corporation at a time, and lapses unused after a week. A syndicate
//! at war with the Authority needs none, as with hull licences.
use serde::{Deserialize, Serialize};

/// A claim unused this long lapses, unrefunded, and the system is for sale
/// again. Tunable.
pub const CLAIM_LAPSE_S: f64 = 7.0 * 86_400.0;
/// Each system already held beyond home, and each claim already held, makes
/// the next claim this much dearer. Tunable.
pub const CLAIM_ESCALATION: f64 = 1.6;

/// One claim a corporation holds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub bought_at: f64,
    pub lapses_at: f64,
}

/// What the Authority asks for a claim on a system whose generated claim cost
/// is `claim_cost` (`galaxy::claim_cost_for`: 600 + 45 × its value-rate), from
/// a corporation holding `beyond_home` systems past its home and `held` claims,
/// at `standing` (the licences' tariff).
pub fn price(claim_cost: f64, beyond_home: usize, held: usize, standing: f64) -> f64 {
    claim_cost * CLAIM_ESCALATION.powi((beyond_home + held) as i32) * crate::tca::tariff_mult(standing)
}

/// Why the Authority refused a claim. Owner-only; a refusal costs nothing
/// (the payment travels home).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ClaimRefusal {
    /// No such system, or it is already a colony.
    NotForSale,
    /// The corporation already holds this claim.
    AlreadyHeld,
    /// Another corporation holds it.
    HeldByAnother,
    /// The charter is Revoked or Proscribed: the Exchange is closed to it.
    CharterClosed,
    /// At war with the Authority: it needs no claims.
    AtWar,
    /// The payment didn't cover it.
    CantAfford { cost: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_system_and_claim_held_makes_the_next_dearer() {
        let full = crate::tca::TCA_STANDING_START;
        let first = price(800.0, 0, 0, full);
        assert!((first - 800.0 * crate::tca::tariff_mult(full)).abs() < 1e-6);
        assert!((price(800.0, 1, 0, full) / first - 1.6).abs() < 1e-9, "a colony held");
        assert!((price(800.0, 0, 1, full) / first - 1.6).abs() < 1e-9, "a claim held counts the same");
        assert!((price(800.0, 2, 1, full) / first - 1.6f64.powi(3)).abs() < 1e-9);
    }
}

//! Passwords use RustCrypto's PHC format: algorithm, cost, random salt and hash.
//! This is one-way Argon2id, not encryption. SHA-256 is used ONLY for random
//! session tokens elsewhere. Increasing these costs preserves old PHC verifiers.
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use rand_core::OsRng;
use zeroize::Zeroizing;

use super::AuthError;

pub(super) fn validate(password: &str) -> Result<(), AuthError> {
    // No trimming, truncation or composition rules: spaces/Unicode and password
    // managers work. Bound bytes as well as characters before any expensive work.
    if !(8..=128).contains(&password.chars().count()) || password.len() > 512 {
        return Err(AuthError::BadInput(
            "Use a password or passphrase of 8–128 characters.",
        ));
    }
    // Reject trivial guesses without requiring arbitrary capitals/digits/symbols.
    // Spaces within a passphrase remain valid and are never stripped from it.
    if password.chars().all(|c| Some(c) == password.chars().next())
        || matches!(
            password.to_lowercase().as_str(),
            "passwordpassword" | "123456789012345" | "1234567890123456" | "qwertyuiopasdfgh"
        )
    {
        return Err(AuthError::BadInput("Choose a less predictable passphrase."));
    }
    Ok(())
}

fn algorithm() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(64 * 1024, 3, 1, None).expect("fixed Argon2id parameters"),
    )
}

pub(super) fn hash(password: Zeroizing<String>) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    algorithm()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::Unavailable)
}

pub(super) fn verify(password: Zeroizing<String>, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        algorithm()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

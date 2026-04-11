use argon2::{Algorithm, Argon2, Params, Version};
use rand::{thread_rng, RngCore};

use crate::error::AuthError;

const SALT_LEN: usize = 16;
const HASH_LEN: usize = 32;

// Lighter than KEK params — targets ~200ms on mobile for interactive use.
const ARGON2_M_COST_KB: u32 = 32 * 1024; // 32 MB
const ARGON2_T_COST: u32 = 2;
const ARGON2_P_COST: u32 = 1;

pub struct PinHash;

impl PinHash {
    /// Validate PIN constraints: 4-6 digits, no trivial patterns.
    pub fn validate(pin: &str) -> Result<(), AuthError> {
        if pin.len() < 4 || pin.len() > 6 {
            return Err(AuthError::InvalidPin("must be 4-6 digits".into()));
        }
        if !pin.chars().all(|c| c.is_ascii_digit()) {
            return Err(AuthError::InvalidPin("must contain only digits".into()));
        }
        // Reject all-same digits (0000, 1111, etc.)
        if pin.chars().all(|c| c == pin.as_bytes()[0] as char) {
            return Err(AuthError::InvalidPin("too simple".into()));
        }
        // Reject ascending/descending sequences (1234, 4321, etc.)
        let digits: Vec<i8> = pin.bytes().map(|b| (b - b'0') as i8).collect();
        let all_ascending = digits.windows(2).all(|w| w[1] - w[0] == 1);
        let all_descending = digits.windows(2).all(|w| w[0] - w[1] == 1);
        if all_ascending || all_descending {
            return Err(AuthError::InvalidPin("too simple".into()));
        }
        Ok(())
    }

    /// Hash a PIN with a random salt. Returns `salt(16) || hash(32)`.
    pub fn hash(pin: &str) -> Result<Vec<u8>, AuthError> {
        let params = Params::new(ARGON2_M_COST_KB, ARGON2_T_COST, ARGON2_P_COST, Some(HASH_LEN))
            .map_err(|e| AuthError::InvalidPin(e.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut salt = [0u8; SALT_LEN];
        thread_rng().fill_bytes(&mut salt);

        let mut hash = [0u8; HASH_LEN];
        argon2
            .hash_password_into(pin.as_bytes(), &salt, &mut hash)
            .map_err(|e| AuthError::InvalidPin(e.to_string()))?;

        let mut result = Vec::with_capacity(SALT_LEN + HASH_LEN);
        result.extend_from_slice(&salt);
        result.extend_from_slice(&hash);
        Ok(result)
    }

    /// Verify a PIN against a stored `salt || hash`.
    pub fn verify(pin: &str, stored: &[u8]) -> Result<bool, AuthError> {
        if stored.len() != SALT_LEN + HASH_LEN {
            return Err(AuthError::PinVerification);
        }
        let salt = &stored[..SALT_LEN];
        let expected = &stored[SALT_LEN..];

        let params = Params::new(ARGON2_M_COST_KB, ARGON2_T_COST, ARGON2_P_COST, Some(HASH_LEN))
            .map_err(|_| AuthError::PinVerification)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut computed = [0u8; HASH_LEN];
        argon2
            .hash_password_into(pin.as_bytes(), salt, &mut computed)
            .map_err(|_| AuthError::PinVerification)?;

        // Constant-time comparison
        Ok(computed
            .iter()
            .zip(expected.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_good_pins() {
        assert!(PinHash::validate("1379").is_ok());
        assert!(PinHash::validate("84920").is_ok());
        assert!(PinHash::validate("482916").is_ok());
    }

    #[test]
    fn validate_rejects_short() {
        assert!(PinHash::validate("123").is_err());
    }

    #[test]
    fn validate_rejects_long() {
        assert!(PinHash::validate("1234567").is_err());
    }

    #[test]
    fn validate_rejects_non_digits() {
        assert!(PinHash::validate("12ab").is_err());
    }

    #[test]
    fn validate_rejects_repeated() {
        assert!(PinHash::validate("0000").is_err());
        assert!(PinHash::validate("1111").is_err());
        assert!(PinHash::validate("55555").is_err());
    }

    #[test]
    fn validate_rejects_sequential() {
        assert!(PinHash::validate("1234").is_err());
        assert!(PinHash::validate("4321").is_err());
        assert!(PinHash::validate("56789").is_err());
    }

    #[test]
    fn hash_and_verify_correct() {
        let stored = PinHash::hash("8523").unwrap();
        assert_eq!(stored.len(), SALT_LEN + HASH_LEN);
        assert!(PinHash::verify("8523", &stored).unwrap());
    }

    #[test]
    fn verify_wrong_pin() {
        let stored = PinHash::hash("8523").unwrap();
        assert!(!PinHash::verify("9999", &stored).unwrap());
    }

    #[test]
    fn different_hashes_for_same_pin() {
        let h1 = PinHash::hash("8523").unwrap();
        let h2 = PinHash::hash("8523").unwrap();
        // Different random salts → different outputs
        assert_ne!(h1, h2);
        // But both verify
        assert!(PinHash::verify("8523", &h1).unwrap());
        assert!(PinHash::verify("8523", &h2).unwrap());
    }
}

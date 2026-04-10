use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::error::AuthError;

/// KEK (Key-Encryption-Key) derivation using Argon2id.
pub struct KekDerivation;

/// Argon2id parameters for KEK derivation.
const ARGON2_M_COST_KB: u32 = 64 * 1024; // 64 MB
const ARGON2_T_COST: u32 = 3; // 3 iterations
const ARGON2_P_COST: u32 = 4; // 4 parallel lanes
const KEK_LEN: usize = 32; // AES-256 key size

/// Fixed app-level context used as the Argon2id password input.
/// All real entropy comes from the hardware-backed salt.
const APP_CONTEXT: &[u8] = b"com.oubli.wallet.kek.v1";

impl KekDerivation {
    /// Derive a 256-bit KEK from a hardware-backed salt using Argon2id.
    ///
    /// The salt must come from the device's hardware-backed keystore
    /// (via `PlatformStorage::generate_hardware_salt`). A fixed app-level
    /// context string is used as the Argon2id password input; all real
    /// entropy comes from the salt.
    pub fn derive_kek(salt: &[u8]) -> Result<Zeroizing<[u8; KEK_LEN]>, AuthError> {
        let params = Params::new(
            ARGON2_M_COST_KB,
            ARGON2_T_COST,
            ARGON2_P_COST,
            Some(KEK_LEN),
        )
        .map_err(|e| AuthError::KekDerivation(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut kek = Zeroizing::new([0u8; KEK_LEN]);
        argon2
            .hash_password_into(APP_CONTEXT, salt, kek.as_mut())
            .map_err(|e| AuthError::KekDerivation(e.to_string()))?;

        Ok(kek)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_kek_deterministic() {
        let salt = b"test_salt_16byte";
        let kek1 = KekDerivation::derive_kek(salt).unwrap();
        let kek2 = KekDerivation::derive_kek(salt).unwrap();
        assert_eq!(kek1.as_ref(), kek2.as_ref());
    }

    #[test]
    fn different_salt_different_kek() {
        let kek1 = KekDerivation::derive_kek(b"salt_aaaaaaa_16b").unwrap();
        let kek2 = KekDerivation::derive_kek(b"salt_bbbbbbb_16b").unwrap();
        assert_ne!(kek1.as_ref(), kek2.as_ref());
    }

    #[test]
    fn kek_is_32_bytes() {
        let kek = KekDerivation::derive_kek(b"test_salt_16byte").unwrap();
        assert_eq!(kek.len(), 32);
    }
}

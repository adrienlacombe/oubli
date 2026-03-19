use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, AeadCore, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::BackupError;

/// Argon2id parameters for cloud backup — heavier than KEK derivation.
const ARGON2_M_COST_KB: u32 = 128 * 1024; // 128 MB
const ARGON2_T_COST: u32 = 4;
const ARGON2_P_COST: u32 = 4;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;

/// Encrypted cloud backup payload: `[salt(16) | nonce(12) | ciphertext]`
#[derive(Debug, Clone)]
pub struct CloudBackupPayload {
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

impl CloudBackupPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + self.ciphertext.len());
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.ciphertext);
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, BackupError> {
        if data.len() < SALT_LEN + NONCE_LEN + 1 {
            return Err(BackupError::Decryption("backup payload too short".into()));
        }
        let salt: [u8; SALT_LEN] = data[..SALT_LEN]
            .try_into()
            .map_err(|_| BackupError::Decryption("invalid salt".into()))?;
        let nonce: [u8; NONCE_LEN] = data[SALT_LEN..SALT_LEN + NONCE_LEN]
            .try_into()
            .map_err(|_| BackupError::Decryption("invalid nonce".into()))?;
        let ciphertext = data[SALT_LEN + NONCE_LEN..].to_vec();
        Ok(Self {
            salt,
            nonce,
            ciphertext,
        })
    }
}

/// Cloud backup encryption/decryption using Argon2id + AES-256-GCM.
pub struct CloudBackup;

impl CloudBackup {
    /// Encrypt a mnemonic for cloud storage using a user-chosen password.
    pub fn encrypt(mnemonic: &str, password: &str) -> Result<CloudBackupPayload, BackupError> {
        let mut salt = [0u8; SALT_LEN];
        use rand::RngCore;
        OsRng.fill_bytes(&mut salt);

        let key = Self::derive_key(password, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| BackupError::Encryption(e.to_string()))?;
        let nonce_val = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce_val, mnemonic.as_bytes())
            .map_err(|e| BackupError::Encryption(e.to_string()))?;

        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(nonce_val.as_slice());

        Ok(CloudBackupPayload {
            salt,
            nonce,
            ciphertext,
        })
    }

    /// Decrypt a cloud backup payload with the user's password.
    pub fn decrypt(payload: &CloudBackupPayload, password: &str) -> Result<String, BackupError> {
        let key = Self::derive_key(password, &payload.salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| BackupError::Decryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&payload.nonce);
        let plaintext = cipher
            .decrypt(nonce, payload.ciphertext.as_slice())
            .map_err(|e| BackupError::Decryption(e.to_string()))?;
        String::from_utf8(plaintext).map_err(|e| BackupError::Decryption(e.to_string()))
    }

    fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], BackupError> {
        let params = Params::new(ARGON2_M_COST_KB, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN))
            .map_err(|e| BackupError::Encryption(e.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; KEY_LEN];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|e| BackupError::Encryption(e.to_string()))?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MNEMONIC: &str =
        "abandon ability able about above absent absorb abstract absurd abuse access accident";

    #[test]
    fn encrypt_decrypt_round_trip() {
        let password = "strong-backup-password-123!";
        let payload = CloudBackup::encrypt(TEST_MNEMONIC, password).unwrap();
        let recovered = CloudBackup::decrypt(&payload, password).unwrap();
        assert_eq!(recovered, TEST_MNEMONIC);
    }

    #[test]
    fn wrong_password_fails() {
        let payload = CloudBackup::encrypt(TEST_MNEMONIC, "correct").unwrap();
        assert!(CloudBackup::decrypt(&payload, "wrong").is_err());
    }

    #[test]
    fn serialize_deserialize_round_trip() {
        let password = "test-password";
        let payload = CloudBackup::encrypt(TEST_MNEMONIC, password).unwrap();
        let bytes = payload.to_bytes();
        let restored = CloudBackupPayload::from_bytes(&bytes).unwrap();
        let recovered = CloudBackup::decrypt(&restored, password).unwrap();
        assert_eq!(recovered, TEST_MNEMONIC);
    }

    #[test]
    fn different_encryptions_differ() {
        let p1 = CloudBackup::encrypt(TEST_MNEMONIC, "pass").unwrap();
        let p2 = CloudBackup::encrypt(TEST_MNEMONIC, "pass").unwrap();
        // Different salt → different ciphertext
        assert_ne!(p1.ciphertext, p2.ciphertext);
    }
}

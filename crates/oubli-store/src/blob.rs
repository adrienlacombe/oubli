use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};

use crate::error::StoreError;

const BLOB_VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

/// An encrypted blob with versioning, nonce, and AAD for integrity.
#[derive(Debug, Clone)]
pub struct EncryptedBlob {
    pub version: u8,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
    pub aad: Vec<u8>,
}

impl EncryptedBlob {
    /// Serialize to bytes: `[version(1) | nonce(12) | aad_len(2) | aad | ciphertext]`
    pub fn to_bytes(&self) -> Vec<u8> {
        let aad_len = (self.aad.len() as u16).to_be_bytes();
        let mut out =
            Vec::with_capacity(1 + NONCE_LEN + 2 + self.aad.len() + self.ciphertext.len());
        out.push(self.version);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&aad_len);
        out.extend_from_slice(&self.aad);
        out.extend_from_slice(&self.ciphertext);
        out
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, StoreError> {
        if data.len() < 1 + NONCE_LEN + 2 {
            return Err(StoreError::Decryption("blob too short".into()));
        }
        let version = data[0];
        if version != BLOB_VERSION {
            return Err(StoreError::VersionMismatch {
                expected: BLOB_VERSION,
                got: version,
            });
        }
        let nonce: [u8; NONCE_LEN] = data[1..1 + NONCE_LEN]
            .try_into()
            .map_err(|_| StoreError::Decryption("invalid nonce".into()))?;
        let aad_len = u16::from_be_bytes([data[1 + NONCE_LEN], data[1 + NONCE_LEN + 1]]) as usize;
        let aad_start = 1 + NONCE_LEN + 2;
        if data.len() < aad_start + aad_len {
            return Err(StoreError::Decryption("blob truncated".into()));
        }
        let aad = data[aad_start..aad_start + aad_len].to_vec();
        let ciphertext = data[aad_start + aad_len..].to_vec();
        Ok(Self {
            version,
            nonce,
            ciphertext,
            aad,
        })
    }
}

/// Build AAD from app_id and version to prevent blob transplant attacks.
pub fn build_aad(app_id: &str, version: u8) -> Vec<u8> {
    let mut aad = Vec::with_capacity(app_id.len() + 1);
    aad.extend_from_slice(app_id.as_bytes());
    aad.push(version);
    aad
}

/// Manages encryption and decryption of blobs using AES-256-GCM.
pub struct BlobManager;

impl BlobManager {
    /// Encrypt plaintext with the given KEK and AAD.
    pub fn wrap(
        kek: &[u8; 32],
        plaintext: &[u8],
        app_id: &str,
    ) -> Result<EncryptedBlob, StoreError> {
        let aad = build_aad(app_id, BLOB_VERSION);
        let cipher =
            Aes256Gcm::new_from_slice(kek).map_err(|e| StoreError::Encryption(e.to_string()))?;
        let nonce_val = Aes256Gcm::generate_nonce(&mut OsRng);
        let payload = aes_gcm::aead::Payload {
            msg: plaintext,
            aad: &aad,
        };
        let ciphertext = cipher
            .encrypt(&nonce_val, payload)
            .map_err(|e| StoreError::Encryption(e.to_string()))?;
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(nonce_val.as_slice());
        Ok(EncryptedBlob {
            version: BLOB_VERSION,
            nonce,
            ciphertext,
            aad,
        })
    }

    /// Decrypt an `EncryptedBlob` with the given KEK, verifying AAD.
    pub fn unwrap(
        kek: &[u8; 32],
        blob: &EncryptedBlob,
        app_id: &str,
    ) -> Result<Vec<u8>, StoreError> {
        let expected_aad = build_aad(app_id, blob.version);
        if blob.aad != expected_aad {
            return Err(StoreError::AadMismatch);
        }
        let cipher =
            Aes256Gcm::new_from_slice(kek).map_err(|e| StoreError::Decryption(e.to_string()))?;
        let nonce = Nonce::from_slice(&blob.nonce);
        let payload = aes_gcm::aead::Payload {
            msg: blob.ciphertext.as_slice(),
            aad: &blob.aad,
        };
        cipher
            .decrypt(nonce, payload)
            .map_err(|e| StoreError::Decryption(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_round_trip() {
        let kek = [0x42u8; 32];
        let plaintext = b"secret mnemonic data";
        let app_id = "com.oubli.wallet";

        let blob = BlobManager::wrap(&kek, plaintext, app_id).unwrap();
        let recovered = BlobManager::unwrap(&kek, &blob, app_id).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_kek_fails() {
        let kek = [0x42u8; 32];
        let wrong_kek = [0x43u8; 32];
        let blob = BlobManager::wrap(&kek, b"secret", "app").unwrap();
        assert!(BlobManager::unwrap(&wrong_kek, &blob, "app").is_err());
    }

    #[test]
    fn wrong_app_id_fails() {
        let kek = [0x42u8; 32];
        let blob = BlobManager::wrap(&kek, b"secret", "app1").unwrap();
        assert!(BlobManager::unwrap(&kek, &blob, "app2").is_err());
    }

    #[test]
    fn serialize_deserialize_round_trip() {
        let kek = [0x42u8; 32];
        let blob = BlobManager::wrap(&kek, b"hello world", "test").unwrap();
        let bytes = blob.to_bytes();
        let restored = EncryptedBlob::from_bytes(&bytes).unwrap();
        let plaintext = BlobManager::unwrap(&kek, &restored, "test").unwrap();
        assert_eq!(plaintext, b"hello world");
    }

    #[test]
    fn version_mismatch() {
        let kek = [0x42u8; 32];
        let blob = BlobManager::wrap(&kek, b"data", "app").unwrap();
        let mut bytes = blob.to_bytes();
        bytes[0] = 99; // corrupt version
        let err = EncryptedBlob::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, StoreError::VersionMismatch { .. }));
    }
}

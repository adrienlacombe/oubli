use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("blob version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u8, got: u8 },

    #[error("platform storage error: {0}")]
    Platform(String),

    #[error("key not found: {0}")]
    NotFound(String),

    #[error("AAD mismatch — possible blob transplant attack")]
    AadMismatch,

    #[error("auth error: {0}")]
    Auth(#[from] oubli_auth::AuthError),
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("seed display flow error: {0}")]
    SeedDisplay(String),

    #[error("verification failed: word at position {position} is incorrect")]
    VerificationFailed { position: usize },

    #[error("cloud backup encryption failed: {0}")]
    Encryption(String),

    #[error("cloud backup decryption failed: {0}")]
    Decryption(String),

    #[error("store error: {0}")]
    Store(#[from] oubli_store::StoreError),
}

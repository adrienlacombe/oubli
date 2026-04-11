use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("auth error: {0}")]
    Auth(#[from] oubli_auth::AuthError),

    #[error("store error: {0}")]
    Store(#[from] oubli_store::StoreError),

    #[error("backup error: {0}")]
    Backup(#[from] oubli_backup::BackupError),

    #[error("kms error: {0}")]
    Kms(String),

    #[error("rpc error: {0}")]
    Rpc(String),

    #[error("paymaster error: {0}")]
    Paymaster(String),

    #[error("invalid state: expected {expected}, got {got}")]
    InvalidState { expected: String, got: String },

    #[error("no active account")]
    NoActiveAccount,

    #[error("denomination error: {0}")]
    Denomination(String),

    #[error("insufficient balance: have {available} tongo units, need {requested}")]
    InsufficientBalance { available: u128, requested: u128 },

    #[error("operation in progress")]
    OperationInProgress,

    #[error("network error: {0}")]
    Network(String),

    #[error("signing error: {0}")]
    Signing(String),

    #[error("typed data validation error: {0}")]
    TypedDataValidation(String),
}

impl From<krusty_kms_common::KmsError> for WalletError {
    fn from(e: krusty_kms_common::KmsError) -> Self {
        WalletError::Kms(e.to_string())
    }
}

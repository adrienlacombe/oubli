use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("transition denied: cannot move from {from:?} via {action:?}")]
    TransitionDenied {
        from: crate::tier::AuthTier,
        action: crate::tier::AuthAction,
    },

    #[error("session expired")]
    SessionExpired,

    #[error("KEK derivation failed: {0}")]
    KekDerivation(String),

    #[error("PIN verification failed")]
    PinVerification,

    #[error("PIN not set")]
    PinNotSet,

    #[error("invalid PIN: {0}")]
    InvalidPin(String),
}

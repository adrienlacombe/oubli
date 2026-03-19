use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("transition denied: cannot move from {from:?} via {action:?}")]
    TransitionDenied { from: crate::tier::AuthTier, action: crate::tier::AuthAction },

    #[error("session expired")]
    SessionExpired,

    #[error("KEK derivation failed: {0}")]
    KekDerivation(String),
}

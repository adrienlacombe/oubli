use thiserror::Error;

#[derive(Debug, Error)]
pub enum SwapError {
    #[error("JS runtime error: {0}")]
    Runtime(String),

    #[error("JS execution error: {0}")]
    Execution(String),

    #[error("Swap not initialized")]
    NotInitialized,

    #[error("Swap failed: {0}")]
    SwapFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for SwapError {
    fn from(e: serde_json::Error) -> Self {
        SwapError::Serialization(e.to_string())
    }
}

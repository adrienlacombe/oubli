use serde::{Deserialize, Serialize};

/// Result wrapper matching the JSON protocol from JS.
#[derive(Debug, Deserialize)]
pub struct JsResult<T> {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub data: Option<T>,
}

/// Quote returned when creating a swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapQuote {
    /// Unique swap identifier.
    pub swap_id: String,
    /// Input amount in base units (sats for BTC, token units for WBTC).
    pub input_amount: String,
    /// Output amount in base units.
    pub output_amount: String,
    /// Total fee in source token units.
    pub fee: String,
    /// Expiry timestamp (unix seconds).
    pub expiry: u64,
    /// BTC address to send to (for BTC→WBTC swaps).
    pub btc_address: Option<String>,
    /// Lightning invoice to pay (for BTCLN→WBTC swaps).
    pub ln_invoice: Option<String>,
}

/// Swap status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapStatus {
    pub swap_id: String,
    pub state: SwapState,
    pub tx_id: Option<String>,
    pub message: Option<String>,
}

/// Simplified swap state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapState {
    Created,
    BtcPending,
    BtcConfirmed,
    Claiming,
    Completed,
    Failed,
    Refundable,
}

/// Swap limits for a given direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapLimits {
    pub input: SwapLimitRange,
    pub output: SwapLimitRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapLimitRange {
    pub min: String,
    pub max: Option<String>,
}

/// Summary of an active swap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapSummary {
    pub swap_id: String,
    pub state: SwapState,
    pub input_amount: String,
    pub output_amount: String,
}

/// Direction of swap.
#[derive(Debug, Clone, Copy)]
pub enum SwapDirection {
    BtcToWbtc,
    WbtcToBtc,
    LnToWbtc,
    WbtcToBtcLn,
}

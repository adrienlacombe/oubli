//! oubli-swap: BTC ↔ WBTC cross-chain swaps via embedded Atomiq SDK.
//!
//! Uses QuickJS (via rquickjs) to run the Atomiq TypeScript SDK in an
//! embedded JS runtime. Starknet signing is delegated back to Rust,
//! keeping private keys secure. HTTP requests use reqwest from Rust.

pub mod error;
pub mod runtime;
pub mod telemetry;
pub mod types;

use std::sync::Arc;

use error::SwapError;
use runtime::{JsRuntime, RuntimeConfig, StarknetSignerCallback, SwapStorage};
use types::*;

/// Main swap engine. Wraps the JS runtime and provides a clean Rust API.
pub struct SwapEngine {
    runtime: JsRuntime,
}

impl SwapEngine {
    /// Create and initialize the swap engine.
    ///
    /// This loads the Atomiq SDK JS bundle into QuickJS, registers host functions
    /// for signing/fetch/storage, and initializes the swapper with LP discovery.
    pub async fn new(
        config: RuntimeConfig,
        signer: Arc<dyn StarknetSignerCallback>,
        storage: Arc<dyn SwapStorage>,
    ) -> Result<Self, SwapError> {
        let runtime = JsRuntime::new(config, signer, storage).await?;

        // Initialize the Atomiq swapper (LP discovery, chain setup)
        let result_json = runtime.call_js_fn("init", &[]).await?;
        let result: serde_json::Value = serde_json::from_str(&result_json)?;

        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(format!("Init failed: {}", err)));
        }

        let lp_count = result.get("lpCount").and_then(|v| v.as_i64()).unwrap_or(-1);
        if lp_count == 0 {
            let rust_fetch_log = runtime::FETCH_LOG
                .lock()
                .map(|log| log.join(" | "))
                .unwrap_or_default();
            return Err(SwapError::SwapFailed(format!(
                "0 LPs. Rust fetch log: {}",
                rust_fetch_log
            )));
        }

        Ok(Self { runtime })
    }

    /// Create a BTC → WBTC on-chain swap.
    /// Returns a quote with a BTC address to send funds to.
    pub async fn create_btc_to_wbtc(
        &self,
        amount_sats: u64,
        exact_in: bool,
    ) -> Result<SwapQuote, SwapError> {
        let result_json = self
            .runtime
            .call_js_fn(
                "createBtcToWbtcSwap",
                &[&amount_sats.to_string(), &exact_in.to_string()],
            )
            .await?;
        self.parse_quote_result(&result_json)
    }

    /// Create a WBTC → BTC off-ramp swap.
    /// Locks WBTC in escrow; LP sends BTC to the provided address.
    pub async fn create_wbtc_to_btc(
        &self,
        amount_sats: u64,
        btc_address: &str,
        exact_in: bool,
    ) -> Result<SwapQuote, SwapError> {
        let result_json = self
            .runtime
            .call_js_fn(
                "createWbtcToBtcSwap",
                &[&amount_sats.to_string(), btc_address, &exact_in.to_string()],
            )
            .await?;
        self.parse_quote_result(&result_json)
    }

    /// Create a Lightning BTC → WBTC swap.
    /// Returns a quote with a Lightning invoice to pay.
    pub async fn create_ln_to_wbtc(
        &self,
        amount_sats: u64,
        exact_in: bool,
    ) -> Result<SwapQuote, SwapError> {
        let result_json = self
            .runtime
            .call_js_fn(
                "createLnToWbtcSwap",
                &[&amount_sats.to_string(), &exact_in.to_string()],
            )
            .await?;
        self.parse_quote_result(&result_json)
    }

    /// Create a WBTC → BTCLN swap (pay a Lightning invoice).
    /// Locks WBTC in escrow; LP pays the Lightning invoice.
    /// Returns a quote with input_amount (WBTC needed).
    pub async fn create_wbtc_to_btc_ln(&self, bolt11: &str) -> Result<SwapQuote, SwapError> {
        let result_json = self
            .runtime
            .call_js_fn("createWbtcToBtcLnSwap", &[bolt11])
            .await?;
        self.parse_quote_result(&result_json)
    }

    /// Wait for an incoming Lightning payment and claim WBTC.
    /// Used for BTCLN → WBTC swaps (receive Lightning).
    /// Blocks until the payer pays the invoice, then claims WBTC from LP escrow.
    pub async fn wait_for_incoming_swap(&self, swap_id: &str) -> Result<(), SwapError> {
        let result_json = self
            .runtime
            .call_js_fn("waitForIncomingSwap", &[swap_id])
            .await?;
        let result: serde_json::Value = serde_json::from_str(&result_json)?;
        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(err.to_string()));
        }
        Ok(())
    }

    /// Execute a pending swap (sign and submit Starknet transactions).
    pub async fn execute_swap(&self, swap_id: &str) -> Result<(), SwapError> {
        let result_json = self.runtime.call_js_fn("executeSwap", &[swap_id]).await?;
        let result: serde_json::Value = serde_json::from_str(&result_json)?;
        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(err.to_string()));
        }
        Ok(())
    }

    /// Get the status of a swap.
    pub async fn get_swap_status(&self, swap_id: &str) -> Result<SwapStatus, SwapError> {
        let result_json = self.runtime.call_js_fn("getSwapStatus", &[swap_id]).await?;
        let result: serde_json::Value = serde_json::from_str(&result_json)?;
        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(err.to_string()));
        }
        let status: SwapStatus =
            serde_json::from_value(result.get("status").cloned().unwrap_or_default())?;
        Ok(status)
    }

    /// Get all active/pending swaps.
    pub async fn get_all_swaps(&self) -> Result<Vec<SwapSummary>, SwapError> {
        let result_json = self.runtime.call_js_fn("getAllSwaps", &[]).await?;
        let result: serde_json::Value = serde_json::from_str(&result_json)?;
        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(err.to_string()));
        }
        let swaps: Vec<SwapSummary> =
            serde_json::from_value(result.get("swaps").cloned().unwrap_or_default())?;
        Ok(swaps)
    }

    /// Get swap limits for a given direction.
    pub async fn get_swap_limits(&self, direction: SwapDirection) -> Result<SwapLimits, SwapError> {
        let dir_str = match direction {
            SwapDirection::BtcToWbtc => "btc_to_wbtc",
            SwapDirection::WbtcToBtc => "wbtc_to_btc",
            SwapDirection::LnToWbtc => "btc_to_wbtc", // same limits
            SwapDirection::WbtcToBtcLn => "wbtc_to_btc", // same limits as wbtc→btc
        };
        let result_json = self.runtime.call_js_fn("getSwapLimits", &[dir_str]).await?;
        let result: serde_json::Value = serde_json::from_str(&result_json)?;
        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(err.to_string()));
        }
        let limits: SwapLimits =
            serde_json::from_value(result.get("limits").cloned().unwrap_or_default())?;
        Ok(limits)
    }

    fn parse_quote_result(&self, json: &str) -> Result<SwapQuote, SwapError> {
        let result: serde_json::Value = serde_json::from_str(json)?;
        if result.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(SwapError::SwapFailed(err.to_string()));
        }
        let quote: SwapQuote =
            serde_json::from_value(result.get("quote").cloned().unwrap_or_default())?;
        Ok(quote)
    }
}

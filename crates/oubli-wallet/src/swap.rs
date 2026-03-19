//! BTC ↔ WBTC swap integration via embedded Atomiq SDK.
//!
//! Bridges oubli-swap's `SwapEngine` into WalletCore by implementing the
//! `StarknetSignerCallback` trait using the wallet's private key.

use std::sync::Arc;

use starknet_types_core::felt::Felt;

use oubli_swap::runtime::{InMemorySwapStorage, RuntimeConfig, StarknetSignerCallback, SwapStorage};
use oubli_swap::{SwapEngine, error::SwapError};

use crate::error::WalletError;
use crate::signing::sign_message_hash;

/// Signer callback that uses a Starknet private key to sign message hashes.
/// The private key is cloned from `ActiveAccount` at swap engine init time.
struct WalletSignerCallback {
    private_key: Felt,
}

impl StarknetSignerCallback for WalletSignerCallback {
    fn sign(&self, message_hash: &str) -> Result<(String, String), String> {
        // Parse hex hash → Felt
        let hash = Felt::from_hex(message_hash).map_err(|e| format!("bad hash: {e}"))?;

        // Sign with the Starknet private key
        let (r, s) = sign_message_hash(&hash, &self.private_key)
            .map_err(|e| format!("sign failed: {e}"))?;

        // Return as 0x-prefixed hex
        Ok((format!("{:#066x}", r), format!("{:#066x}", s)))
    }
}

/// Initialize the swap engine from wallet state.
/// Called lazily when the first swap operation is requested.
pub(crate) async fn create_swap_engine(
    starknet_address: &Felt,
    starknet_public_key: &Felt,
    starknet_private_key: &Felt,
    chain_id: &str,
    rpc_url: &str,
    account_class_hash: &str,
    paymaster_url: &str,
    paymaster_api_key: Option<&str>,
) -> Result<SwapEngine, WalletError> {
    let config = RuntimeConfig {
        starknet_address: format!("{:#066x}", starknet_address),
        starknet_public_key: format!("{:#066x}", starknet_public_key),
        starknet_chain_id: chain_id.to_string(),
        starknet_rpc_url: rpc_url.to_string(),
        account_class_hash: account_class_hash.to_string(),
        paymaster_url: paymaster_url.to_string(),
        paymaster_api_key: paymaster_api_key.map(String::from),
    };

    let signer: Arc<dyn StarknetSignerCallback> = Arc::new(WalletSignerCallback {
        private_key: *starknet_private_key,
    });

    let storage: Arc<dyn SwapStorage> = Arc::new(InMemorySwapStorage::default());

    SwapEngine::new(config, signer, storage)
        .await
        .map_err(|e| WalletError::Network(format!("swap engine init: {e}")))
}

/// Convert SwapError to WalletError.
pub(crate) fn swap_err(e: SwapError) -> WalletError {
    WalletError::Network(format!("swap: {e}"))
}

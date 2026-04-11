//! BTC ↔ WBTC swap integration via embedded Atomiq SDK.
//!
//! Bridges oubli-swap's `SwapEngine` into WalletCore by implementing the
//! `StarknetSignerCallback` trait using the wallet's private key.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use starknet_types_core::felt::Felt;

use oubli_store::PlatformStorage;
use oubli_swap::runtime::{RuntimeConfig, StarknetSignerCallback, SwapStorage};
use oubli_swap::{error::SwapError, SwapEngine};

use crate::error::WalletError;
use crate::signing::sign_message_hash;

/// Signer callback that uses a Starknet private key to sign message hashes.
/// The private key is cloned from `ActiveAccount` at swap engine init time.
struct WalletSignerCallback {
    address: Felt,
    chain_id: String,
    private_key: Felt,
}

impl StarknetSignerCallback for WalletSignerCallback {
    fn sign(&self, message_hash: &str) -> Result<(String, String), String> {
        // Parse hex hash → Felt
        let hash = Felt::from_hex(message_hash).map_err(|e| format!("bad hash: {e}"))?;

        // Sign with the Starknet private key
        let (r, s) =
            sign_message_hash(&hash, &self.private_key).map_err(|e| format!("sign failed: {e}"))?;

        // Return as 0x-prefixed hex
        Ok((format!("{:#066x}", r), format!("{:#066x}", s)))
    }

    fn sign_paymaster_invoke(
        &self,
        typed_data_json: &str,
        expected_calls_json: &str,
    ) -> Result<(String, String), String> {
        let typed_data: serde_json::Value =
            serde_json::from_str(typed_data_json).map_err(|e| format!("parse typed_data: {e}"))?;
        let expected_calls: Vec<serde_json::Value> = serde_json::from_str(expected_calls_json)
            .map_err(|e| format!("parse expected calls: {e}"))?;

        let (r, s) = crate::signing::sign_validated_paymaster_invoke(
            &typed_data,
            &expected_calls,
            &self.address,
            &self.chain_id,
            &self.private_key,
        )
        .map_err(|e| format!("sign paymaster invoke failed: {e}"))?;

        Ok((format!("{:#066x}", r), format!("{:#066x}", s)))
    }
}

/// Platform-backed swap storage that survives app restarts.
struct PersistentSwapStorage {
    storage: Arc<dyn PlatformStorage>,
    storage_key: String,
    io_lock: Mutex<()>,
}

impl PersistentSwapStorage {
    fn new(storage: Arc<dyn PlatformStorage>, chain_id: &str, starknet_address: &Felt) -> Self {
        Self {
            storage,
            storage_key: format!(
                "oubli.swap.{}.{}",
                chain_id,
                format!("{:#066x}", starknet_address)
            ),
            io_lock: Mutex::new(()),
        }
    }

    fn load_map(&self) -> HashMap<String, String> {
        match self.storage.secure_load(&self.storage_key) {
            Ok(Some(bytes)) => match serde_json::from_slice(&bytes) {
                Ok(map) => map,
                Err(e) => {
                    crate::warn_event!(
                        "wallet.swap_storage",
                        "decode_failed",
                        "error_kind" = crate::telemetry::error_kind(&e)
                    );
                    HashMap::new()
                }
            },
            Ok(None) => HashMap::new(),
            Err(e) => {
                crate::warn_event!(
                    "wallet.swap_storage",
                    "load_failed",
                    "error_kind" = crate::telemetry::error_kind(&e)
                );
                HashMap::new()
            }
        }
    }

    fn store_map(&self, map: &HashMap<String, String>) {
        if map.is_empty() {
            if let Err(e) = self.storage.secure_delete(&self.storage_key) {
                crate::warn_event!(
                    "wallet.swap_storage",
                    "delete_failed",
                    "error_kind" = crate::telemetry::error_kind(&e)
                );
            }
            return;
        }

        match serde_json::to_vec(map) {
            Ok(bytes) => {
                if let Err(e) = self.storage.secure_store(&self.storage_key, &bytes) {
                    crate::warn_event!(
                        "wallet.swap_storage",
                        "store_failed",
                        "error_kind" = crate::telemetry::error_kind(&e)
                    );
                }
            }
            Err(e) => crate::warn_event!(
                "wallet.swap_storage",
                "encode_failed",
                "error_kind" = crate::telemetry::error_kind(&e)
            ),
        }
    }
}

impl SwapStorage for PersistentSwapStorage {
    fn get(&self, key: &str) -> Option<String> {
        let _guard = self.io_lock.lock().unwrap();
        self.load_map().get(key).cloned()
    }

    fn set(&self, key: &str, value: &str) {
        let _guard = self.io_lock.lock().unwrap();
        let mut map = self.load_map();
        map.insert(key.to_string(), value.to_string());
        self.store_map(&map);
    }

    fn remove(&self, key: &str) {
        let _guard = self.io_lock.lock().unwrap();
        let mut map = self.load_map();
        map.remove(key);
        self.store_map(&map);
    }
}

/// Initialize the swap engine from wallet state.
/// Called lazily when the first swap operation is requested.
pub(crate) async fn create_swap_engine(
    storage: Arc<dyn PlatformStorage>,
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
        address: *starknet_address,
        chain_id: chain_id.to_string(),
        private_key: *starknet_private_key,
    });

    let storage: Arc<dyn SwapStorage> = Arc::new(PersistentSwapStorage::new(
        storage,
        chain_id,
        starknet_address,
    ));

    SwapEngine::new(config, signer, storage)
        .await
        .map_err(|e| WalletError::Network(format!("swap engine init: {e}")))
}

/// Convert SwapError to WalletError.
pub(crate) fn swap_err(e: SwapError) -> WalletError {
    WalletError::Network(format!("swap: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oubli_store::MockPlatformStorage;

    #[test]
    fn persistent_swap_storage_survives_recreation() {
        let storage: Arc<dyn PlatformStorage> = Arc::new(MockPlatformStorage::new());
        let address = Felt::from_hex("0x123").unwrap();

        let first = PersistentSwapStorage::new(Arc::clone(&storage), "SN_SEPOLIA", &address);
        first.set("swap-1", "{\"state\":\"created\"}");

        let second = PersistentSwapStorage::new(Arc::clone(&storage), "SN_SEPOLIA", &address);
        assert_eq!(
            second.get("swap-1").as_deref(),
            Some("{\"state\":\"created\"}")
        );

        second.remove("swap-1");

        let third = PersistentSwapStorage::new(storage, "SN_SEPOLIA", &address);
        assert_eq!(third.get("swap-1"), None);
    }
}

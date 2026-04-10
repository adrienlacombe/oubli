use starknet_types_core::curve::ProjectivePoint;
use starknet_types_core::felt::Felt;
use std::sync::Arc;

use oubli_auth::{
    AuthAction, AuthState, AuthTier, AuthTransitionResult, KekDerivation, SessionConfig,
};
use oubli_store::{BlobManager, EncryptedBlob, PlatformStorage};

use crate::config::NetworkConfig;
use crate::denomination::{format_sats_display, tongo_units_to_sats};
use crate::error::WalletError;
use crate::queue::OperationQueue;
use crate::rpc::RpcClient;
use crate::state::WalletState;
use crate::submitter::{PaymasterSubmitter, TransactionSubmitter};

// ── ActivityEvent ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ActivityStatus {
    #[default]
    Unknown,
    Pending,
    Confirmed,
    Failed,
}

impl ActivityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ActivityStatus::Unknown => "Unknown",
            ActivityStatus::Pending => "Pending",
            ActivityStatus::Confirmed => "Confirmed",
            ActivityStatus::Failed => "Failed",
        }
    }
}

/// Simplified event for display in the UI activity list.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivityEvent {
    pub event_type: String,
    pub amount_sats: Option<String>,
    pub tx_hash: String,
    pub block_number: u64,
    #[serde(default)]
    pub timestamp_secs: Option<u64>,
    #[serde(default)]
    pub status: ActivityStatus,
}

impl ActivityEvent {
    pub(crate) fn normalize(&mut self) {
        if self.status == ActivityStatus::Unknown {
            self.status = if self.block_number == 0 {
                ActivityStatus::Pending
            } else {
                ActivityStatus::Confirmed
            };
        }
    }
}

/// Metadata stored locally for each transfer/withdraw tx.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransferMeta {
    amount_sats: String,
    recipient: Option<String>,
}

// ── Storage key constants ────────────────────────────────────

const SEED_BLOB_KEY: &str = "oubli.seed.blob";
const KEK_SALT_KEY: &str = "oubli.kek.salt";
const PUBKEY_KEY: &str = "oubli.pubkey";
const STARKNET_ADDR_KEY: &str = "oubli.starknet.addr";
const ACTIVITY_CACHE_KEY: &str = "oubli.activity.cache";
const TRANSFER_AMOUNTS_KEY: &str = "oubli.transfer.amounts";
const BIRTHDAY_BLOCK_KEY: &str = "oubli.birthday.block";
const APP_ID: &str = "com.oubli.wallet";
const DEVNET_CUSTOM_ACCOUNT_CLASS_HASH: &str =
    "0x05b4b537eaa2399e3aa99c4e2e0208ebd6c71bc1467938cd52c798c601e43564";

// ── ActiveAccount ────────────────────────────────────────────

/// In-memory secrets — dropped on lock. `TongoAccount` zeroizes its `SecretFelt` on drop.
pub struct ActiveAccount {
    pub tongo_account: krusty_kms_sdk::TongoAccount,
    pub starknet_private_key: Felt,
    pub starknet_address: Felt,
    /// Starknet account public key (x-coordinate), used for deploy and display.
    pub starknet_public_key: Felt,
    pub owner_public_key_hex: String,
    pub balance: u128,
    pub pending: u128,
    pub nonce: Felt,
    pub cipher_balance: Option<krusty_kms_client::CipherBalance>,
    pub auditor_key: Option<ProjectivePoint>,
}

// ── WalletCore ───────────────────────────────────────────────

pub struct WalletCore {
    auth_state: AuthState,
    storage: Arc<dyn PlatformStorage>,
    config: NetworkConfig,
    rpc: Option<RpcClient>,
    submitter: Box<dyn TransactionSubmitter>,
    queue: OperationQueue,
    active_account: Option<ActiveAccount>,
    wallet_state: WalletState,
    owner_public_key_hex: Option<String>,
    starknet_address_hex: Option<String>,
    /// Last error from auto-fund (deployment/sweep). Surfaced in UI for debugging.
    last_auto_fund_error: Option<String>,
    /// BTC ↔ WBTC swap engine (lazy-initialized on first swap request).
    swap_engine: Option<oubli_swap::SwapEngine>,
    /// Cached BTC/USD price and fetch time.
    btc_price_cache: Option<(f64, std::time::Instant)>,
}

impl WalletCore {
    pub fn new(storage: Box<dyn PlatformStorage>, config: NetworkConfig) -> Self {
        let submitter = Box::new(PaymasterSubmitter::new(
            &config.paymaster_url,
            config.paymaster_api_key.as_deref(),
        ));
        Self::new_with_submitter(storage, config, submitter)
    }

    pub fn new_with_submitter(
        storage: Box<dyn PlatformStorage>,
        config: NetworkConfig,
        submitter: Box<dyn TransactionSubmitter>,
    ) -> Self {
        let storage: Arc<dyn PlatformStorage> = storage.into();

        // Check if we have stored keys (returning user vs first launch)
        let owner_public_key_hex = storage
            .secure_load(PUBKEY_KEY)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok());
        let starknet_address_hex = storage
            .secure_load(STARKNET_ADDR_KEY)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok());

        let initial_state = if owner_public_key_hex.is_some() {
            WalletState::Locked
        } else {
            WalletState::Onboarding
        };

        Self {
            auth_state: AuthState::new(SessionConfig::default()),
            storage,
            config,
            rpc: None,
            submitter,
            queue: OperationQueue::new(),
            active_account: None,
            wallet_state: initial_state,
            owner_public_key_hex,
            starknet_address_hex,
            last_auto_fund_error: None,
            swap_engine: None,
            btc_price_cache: None,
        }
    }

    // ── BTC price ────────────────────────────────────────────

    /// Get cached BTC/USD price, fetching from API if stale (>1 hour).
    pub async fn get_btc_price_usd(&mut self) -> Option<f64> {
        if let Some((price, fetched_at)) = &self.btc_price_cache {
            if fetched_at.elapsed() < std::time::Duration::from_secs(3600) {
                return Some(*price);
            }
        }
        match fetch_btc_price().await {
            Some(price) => {
                self.btc_price_cache = Some((price, std::time::Instant::now()));
                Some(price)
            }
            None => self.btc_price_cache.as_ref().map(|(p, _)| *p),
        }
    }

    // ── Accessors ────────────────────────────────────────────

    pub fn state(&self) -> &WalletState {
        &self.wallet_state
    }

    pub fn last_auto_fund_error(&self) -> Option<&str> {
        self.last_auto_fund_error.as_deref()
    }

    pub fn owner_public_key(&self) -> Option<&str> {
        self.owner_public_key_hex.as_deref()
    }

    pub fn auth_tier(&self) -> AuthTier {
        self.auth_state.tier
    }

    pub fn active_account(&self) -> Option<&ActiveAccount> {
        self.active_account.as_ref()
    }

    pub fn active_account_mut(&mut self) -> Option<&mut ActiveAccount> {
        self.active_account.as_mut()
    }

    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    pub fn storage(&self) -> &Arc<dyn PlatformStorage> {
        &self.storage
    }

    pub fn rpc_url(&self) -> &str {
        &self.config.rpc_url
    }

    pub fn update_rpc_url(&mut self, new_url: String) -> Result<(), WalletError> {
        self.config.set_rpc_url(&new_url)?;
        self.rpc = None;
        self.init_rpc()?; // recreate immediately with new URL
        Ok(())
    }

    pub fn rpc(&self) -> Option<&RpcClient> {
        self.rpc.as_ref()
    }

    pub fn submitter(&self) -> &dyn TransactionSubmitter {
        &*self.submitter
    }

    pub fn queue_mut(&mut self) -> &mut OperationQueue {
        &mut self.queue
    }

    // ── Onboarding ───────────────────────────────────────────

    pub async fn handle_onboarding(&mut self, mnemonic: &str) -> Result<(), WalletError> {
        // 1. Validate mnemonic
        krusty_kms::validate_mnemonic(mnemonic).map_err(|e| WalletError::Kms(e.to_string()))?;

        // 2. Generate salt and store it
        let salt = self
            .storage
            .generate_hardware_salt()
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?;
        self.storage
            .secure_store(KEK_SALT_KEY, &salt)
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?;

        // 3. Derive KEK and wrap mnemonic
        let kek = KekDerivation::derive_kek(&salt)?;
        let blob = BlobManager::wrap(&kek, mnemonic.as_bytes(), APP_ID)?;
        self.storage
            .secure_store(SEED_BLOB_KEY, &blob.to_bytes())
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?;

        // 4. Derive active account
        let account = derive_active_account(mnemonic, &self.config)?;

        // 5. Store non-secret display data
        self.owner_public_key_hex = Some(account.owner_public_key_hex.clone());
        self.starknet_address_hex = Some(format!("{:#066x}", account.starknet_address));
        self.storage
            .secure_store(PUBKEY_KEY, account.owner_public_key_hex.as_bytes())
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?;
        self.storage
            .secure_store(
                STARKNET_ADDR_KEY,
                self.starknet_address_hex.as_ref().unwrap().as_bytes(),
            )
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?;

        // 6. Init RPC and fetch balance
        self.init_rpc()?;
        let address_display = self.starknet_address_hex.clone().unwrap_or_default();
        self.active_account = Some(account);

        // 6b. Store birthday block (once, on first onboarding only)
        let has_birthday = self
            .storage
            .secure_load(BIRTHDAY_BLOCK_KEY)
            .ok()
            .flatten()
            .is_some();
        if !has_birthday {
            if let Some(rpc) = &self.rpc {
                if let Ok(block) = rpc.fetch_current_block_number().await {
                    let _ = self
                        .storage
                        .secure_store(BIRTHDAY_BLOCK_KEY, block.to_string().as_bytes());
                }
            }
        }

        if let (Some(rpc), Some(ref mut acct)) = (&self.rpc, &mut self.active_account) {
            let _ = fetch_and_decrypt_balance(rpc, acct).await;
        }

        // 7. Promote auth to T2
        self.auth_state.apply(AuthAction::BiometricSuccess);

        // 7b. Auto-fund: sweep any public WBTC into the privacy pool
        self.handle_auto_fund().await;

        // 7c. Auto-rollover: claim any pending balance into spendable
        self.handle_auto_rollover().await;

        // 8. Set wallet state
        let (balance_sats, pending_sats) = self.format_balance();
        self.wallet_state = WalletState::Ready {
            address: address_display,
            balance_sats,
            pending_sats,
        };

        Ok(())
    }

    // ── Biometric unlock (T0 → T2) ──────────────────────────

    pub async fn handle_unlock_biometric(&mut self) -> Result<(), WalletError> {
        eprintln!("[oubli] unlock_biometric: start");
        let bio_ok = self
            .storage
            .request_biometric("Unlock Oubli wallet")
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?;
        if !bio_ok {
            return Err(WalletError::InvalidState {
                expected: "biometric success".into(),
                got: "biometric authentication failed".into(),
            });
        }
        eprintln!("[oubli] unlock_biometric: bio ok");

        match self.auth_state.apply(AuthAction::BiometricSuccess) {
            AuthTransitionResult::TierChanged(_) => {}
            AuthTransitionResult::Denied => {
                return Err(WalletError::InvalidState {
                    expected: "T0Locked".into(),
                    got: format!("{:?}", self.auth_state.tier),
                });
            }
        }
        eprintln!("[oubli] unlock_biometric: auth state updated");

        // Decrypt seed and derive account
        let salt = self
            .storage
            .secure_load(KEK_SALT_KEY)
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?
            .ok_or_else(|| WalletError::NoActiveAccount)?;

        let blob_bytes = self
            .storage
            .secure_load(SEED_BLOB_KEY)
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?
            .ok_or_else(|| WalletError::NoActiveAccount)?;

        let kek = KekDerivation::derive_kek(&salt)?;
        let blob = EncryptedBlob::from_bytes(&blob_bytes)?;
        let seed_bytes = BlobManager::unwrap(&kek, &blob, APP_ID)?;
        let mnemonic =
            String::from_utf8(seed_bytes).map_err(|e| WalletError::Kms(e.to_string()))?;
        eprintln!("[oubli] unlock_biometric: seed decrypted");

        let account = derive_active_account(&mnemonic, &self.config)?;
        eprintln!("[oubli] unlock_biometric: account derived");
        let address = self.starknet_address_hex.clone().unwrap_or_default();
        self.active_account = Some(account);

        // Init RPC and fetch balance
        self.init_rpc()?;
        eprintln!("[oubli] unlock_biometric: rpc init ok");
        if let (Some(rpc), Some(ref mut acct)) = (&self.rpc, &mut self.active_account) {
            let res = fetch_and_decrypt_balance(rpc, acct).await;
            eprintln!("[oubli] unlock_biometric: fetch_balance result = {:?}", res);
        }

        // Auto-fund: sweep any public WBTC into the privacy pool
        eprintln!("[oubli] unlock_biometric: starting auto_fund");
        self.handle_auto_fund().await;
        eprintln!("[oubli] unlock_biometric: auto_fund done");

        // Auto-rollover: claim any pending balance into spendable
        self.handle_auto_rollover().await;

        let (balance_sats, pending_sats) = self.format_balance();
        self.wallet_state = WalletState::Ready {
            address,
            balance_sats,
            pending_sats,
        };
        eprintln!("[oubli] unlock_biometric: state set to Ready");

        Ok(())
    }

    // ── Lock / Background ────────────────────────────────────

    pub fn handle_lock(&mut self) {
        self.active_account = None;
        self.rpc = None;
        self.swap_engine = None;
        self.queue.clear();
        self.auth_state.apply(AuthAction::Lock);
        if self.owner_public_key_hex.is_some() {
            self.wallet_state = WalletState::Locked;
        } else {
            self.wallet_state = WalletState::Onboarding;
        }
    }

    pub fn handle_background(&mut self) {
        self.active_account = None;
        self.rpc = None;
        self.swap_engine = None;
        self.queue.clear();
        self.auth_state.apply(AuthAction::Background);
        if self.owner_public_key_hex.is_some() {
            self.wallet_state = WalletState::Locked;
        } else {
            self.wallet_state = WalletState::Onboarding;
        }
    }

    // ── Refresh balance ──────────────────────────────────────

    pub async fn handle_refresh_balance(&mut self) -> Result<(), WalletError> {
        let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
        let acct = self
            .active_account
            .as_mut()
            .ok_or(WalletError::NoActiveAccount)?;

        fetch_and_decrypt_balance(rpc, acct).await?;

        // Auto-fund: sweep any public WBTC into the privacy pool
        self.handle_auto_fund().await;

        // Auto-rollover: claim any pending balance into spendable
        self.handle_auto_rollover().await;

        // Update wallet state
        let address = self.starknet_address_hex.clone().unwrap_or_default();
        let (balance_sats, pending_sats) = self.format_balance();
        self.wallet_state = WalletState::Ready {
            address,
            balance_sats,
            pending_sats,
        };

        Ok(())
    }

    // ── Activity events ───────────────────────────────────────

    pub async fn get_activity(&self) -> Result<Vec<ActivityEvent>, WalletError> {
        let acct = self
            .active_account
            .as_ref()
            .ok_or(WalletError::NoActiveAccount)?;
        let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;

        let pub_key = &acct.tongo_account.keypair.public_key;
        let priv_key = acct.tongo_account.keypair.private_key.expose_secret();
        let cached = self.get_cached_activity();
        let birthday_block = self
            .storage
            .secure_load(BIRTHDAY_BLOCK_KEY)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| s.parse::<u64>().ok());
        match rpc
            .get_recent_activity(pub_key, Some(priv_key), &cached, birthday_block)
            .await
        {
            Ok(mut events) => {
                // Fill in missing amounts from local storage (for transfers
                // made before on-chain hints were generated).
                self.fill_stored_amounts(&mut events);
                // Cache to persistent storage (best-effort)
                if let Ok(json) = serde_json::to_vec(&events) {
                    let _ = self.storage.secure_store(ACTIVITY_CACHE_KEY, &json);
                }
                Ok(events)
            }
            Err(e) => {
                // Fall back to cached activity on RPC failure
                if !cached.is_empty() {
                    return Ok(cached);
                }
                Err(e)
            }
        }
    }

    /// Load cached activity without making any RPC call.
    pub fn get_cached_activity(&self) -> Vec<ActivityEvent> {
        let mut events: Vec<ActivityEvent> = self
            .storage
            .secure_load(ACTIVITY_CACHE_KEY)
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        normalize_activity_events(&mut events);
        self.fill_stored_amounts(&mut events);
        events
    }

    /// Store transfer metadata (amount + recipient) locally.
    fn store_transfer_meta(&self, tx_hash: &str, amount_sats: &str, recipient: Option<&str>) {
        let mut metas = self.load_transfer_metas();
        metas.insert(
            tx_hash.to_string(),
            TransferMeta {
                amount_sats: amount_sats.to_string(),
                recipient: recipient.map(|s| s.to_string()),
            },
        );
        if let Ok(json) = serde_json::to_vec(&metas) {
            let _ = self.storage.secure_store(TRANSFER_AMOUNTS_KEY, &json);
        }
    }

    /// Load locally-stored transfer metadata.
    /// Backward-compatible: migrates old HashMap<String, String> format.
    fn load_transfer_metas(&self) -> std::collections::HashMap<String, TransferMeta> {
        let bytes = match self.storage.secure_load(TRANSFER_AMOUNTS_KEY) {
            Ok(Some(b)) => b,
            _ => return std::collections::HashMap::new(),
        };

        // Try new format first
        if let Ok(metas) =
            serde_json::from_slice::<std::collections::HashMap<String, TransferMeta>>(&bytes)
        {
            return metas;
        }

        // Fall back to old format: HashMap<String, String> (tx_hash → amount_sats)
        if let Ok(old) = serde_json::from_slice::<std::collections::HashMap<String, String>>(&bytes)
        {
            let metas: std::collections::HashMap<String, TransferMeta> = old
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        TransferMeta {
                            amount_sats: v,
                            recipient: None,
                        },
                    )
                })
                .collect();
            // Migrate: write back in new format
            if let Ok(json) = serde_json::to_vec(&metas) {
                let _ = self.storage.secure_store(TRANSFER_AMOUNTS_KEY, &json);
            }
            return metas;
        }

        std::collections::HashMap::new()
    }

    /// Fill in missing amounts on TransferOut events from local storage.
    fn fill_stored_amounts(&self, events: &mut [ActivityEvent]) {
        let metas = self.load_transfer_metas();
        if metas.is_empty() {
            return;
        }
        for ev in events.iter_mut() {
            if ev.amount_sats.is_none() && ev.event_type == "TransferOut" {
                if let Some(meta) = metas.get(&ev.tx_hash) {
                    ev.amount_sats = Some(meta.amount_sats.clone());
                }
            }
        }
    }

    /// Get the recipient address/pubkey for a given transaction hash.
    pub fn get_transfer_recipient(&self, tx_hash: &str) -> Option<String> {
        self.load_transfer_metas()
            .get(tx_hash)
            .and_then(|m| m.recipient.clone())
    }

    /// Add an optimistic activity event so the UI shows it immediately,
    /// before the transaction is confirmed on-chain.
    fn add_optimistic_activity(&self, event_type: &str, amount_sats: &str, tx_hash: &str) {
        let mut cached = self.get_cached_activity();
        let event = ActivityEvent {
            event_type: event_type.to_string(),
            amount_sats: Some(amount_sats.to_string()),
            tx_hash: tx_hash.to_string(),
            block_number: 0, // pending — will be replaced by on-chain event
            timestamp_secs: Some(now_unix_secs()),
            status: ActivityStatus::Pending,
        };
        cached.insert(0, event);
        cached.truncate(20);
        if let Ok(json) = serde_json::to_vec(&cached) {
            let _ = self.storage.secure_store(ACTIVITY_CACHE_KEY, &json);
        }
    }

    // ── Set state for processing ─────────────────────────────

    pub fn set_processing(&mut self, operation: &str) {
        let address = self.starknet_address_hex.clone().unwrap_or_default();
        self.wallet_state = WalletState::Processing {
            address,
            operation: operation.to_string(),
        };
    }

    pub fn set_ready(&mut self) {
        let address = self.starknet_address_hex.clone().unwrap_or_default();
        let (balance_sats, pending_sats) = self.format_balance();
        self.wallet_state = WalletState::Ready {
            address,
            balance_sats,
            pending_sats,
        };
    }

    pub fn set_error(&mut self, message: String) {
        self.wallet_state = WalletState::Error { message };
    }

    // ── Require T2 for operations ────────────────────────────

    pub fn require_t2(&self) -> Result<&ActiveAccount, WalletError> {
        if self.auth_state.tier != AuthTier::T2Transact
            && self.auth_state.tier != AuthTier::T3Critical
        {
            return Err(WalletError::InvalidState {
                expected: "T2Transact or T3Critical".into(),
                got: format!("{:?}", self.auth_state.tier),
            });
        }
        self.active_account
            .as_ref()
            .ok_or(WalletError::NoActiveAccount)
    }

    /// Deploy the Starknet account via paymaster if not yet on-chain.
    /// No-op if already deployed. Waits for the deploy tx to confirm.
    async fn ensure_account_deployed(&mut self) -> Result<(), WalletError> {
        let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
        let acct = self
            .active_account
            .as_ref()
            .ok_or(WalletError::NoActiveAccount)?;
        let config = self.config.clone();
        let tx_hash = self.submitter.deploy_account(acct, &config, rpc).await?;
        if let Some(hash) = tx_hash {
            self.wait_for_tx(&hash).await?;
        }
        Ok(())
    }

    /// Poll until a transaction is confirmed on-chain (ACCEPTED_ON_L2/L1).
    /// Times out after ~30 seconds.
    async fn wait_for_tx(&self, tx_hash: &str) -> Result<(), WalletError> {
        let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
        for _ in 0..15 {
            if rpc.is_tx_confirmed(tx_hash).await? {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        Err(WalletError::Rpc(format!(
            "tx {tx_hash} not confirmed after 30s"
        )))
    }

    // ── Auto-fund (sweep public WBTC into Tongo) ──────────────

    /// Silently sweep any public WBTC balance into the Tongo privacy pool.
    /// If the Starknet account is not deployed, deploy it first and defer the
    /// fund to the next refresh cycle.
    /// Best-effort: all errors are swallowed and retried on next refresh.
    async fn handle_auto_fund(&mut self) {
        if self.config.paymaster_url.trim().is_empty() {
            // Without sponsored execution, public token balance is also the gas budget on
            // devnet/local flows, so auto-funding would strand the account.
            self.last_auto_fund_error = None;
            return;
        }

        let result: Result<Option<String>, WalletError> = async {
            let token_contract = Felt::from_hex(&self.config.token_contract)
                .map_err(|e| WalletError::Rpc(format!("invalid token contract: {e}")))?;

            let starknet_address = self
                .active_account
                .as_ref()
                .ok_or(WalletError::NoActiveAccount)?
                .starknet_address;

            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let token_balance = rpc
                .get_erc20_balance(&token_contract, &starknet_address)
                .await?;

            let has_auditor = self.active_account.as_ref().map(|a| a.auditor_key.is_some()).unwrap_or(false);
            let has_cipher = self.active_account.as_ref().map(|a| a.cipher_balance.is_some()).unwrap_or(false);
            eprintln!("[oubli] auto_fund: token_balance={token_balance} auditor={has_auditor} cipher={has_cipher}");

            if token_balance == 0 {
                return Ok(None);
            }

            // Deploy the Starknet account if needed (standalone, before fund).
            // If a deploy was just submitted, return early — the fund will
            // succeed on the next refresh once the deploy is confirmed.
            {
                let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
                let acct = self
                    .active_account
                    .as_ref()
                    .ok_or(WalletError::NoActiveAccount)?;
                let config = self.config.clone();
                let deployed = self.submitter.deploy_account(acct, &config, rpc).await?;
                if deployed.is_some() {
                    return Ok(None);
                }
            }

            let rate = rpc
                .contract()
                .get_rate()
                .await
                .map_err(|e| WalletError::Rpc(e.to_string()))?;

            if rate == 0 {
                return Ok(None);
            }

            let tongo_units = token_balance / rate;
            if tongo_units == 0 {
                return Ok(None);
            }

            let amount_sats = tongo_units_to_sats(tongo_units as u64);
            // Skip if dust is too small to represent
            if amount_sats == "0" {
                return Ok(None);
            }
            let config = self.config.clone();
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            let tx_hash = crate::operations::execute_fund(acct, &amount_sats, &config, rpc, submitter).await?;

            Ok(Some(tx_hash))
        }
        .await;

        match result {
            Ok(Some(tx_hash)) => {
                self.last_auto_fund_error = None;
                // Wait for confirmation, then re-fetch so cipher_balance is correct
                eprintln!("[oubli] auto_fund: waiting for tx {tx_hash} confirmation");
                if let Err(e) = self.wait_for_tx(&tx_hash).await {
                    eprintln!("[oubli] auto_fund: wait failed: {e}");
                }
                if let (Some(rpc), Some(ref mut acct)) = (&self.rpc, &mut self.active_account) {
                    let _ = fetch_and_decrypt_balance(rpc, acct).await;
                }
            }
            Ok(None) => {
                self.last_auto_fund_error = None;
            }
            Err(e) => {
                eprintln!("[oubli] auto_fund: error: {e}");
                self.last_auto_fund_error = Some(e.to_string());
            }
        }
    }

    // ── Auto-rollover (claim pending into spendable) ────────

    /// Automatically roll over any pending balance so the user sees it as
    /// spendable without manual intervention.
    /// Best-effort: errors are logged but swallowed (retried on next refresh).
    async fn handle_auto_rollover(&mut self) {
        let has_pending = self
            .active_account
            .as_ref()
            .map(|a| a.pending > 0)
            .unwrap_or(false);

        if !has_pending {
            return;
        }

        eprintln!("[oubli] auto_rollover: pending > 0, triggering rollover");
        let config = self.config.clone();
        let result = {
            let rpc = match self.rpc.as_ref() {
                Some(r) => r,
                None => return,
            };
            let submitter = &*self.submitter;
            let acct = match self.active_account.as_mut() {
                Some(a) => a,
                None => return,
            };
            crate::operations::execute_rollover(acct, &config, rpc, submitter).await
        };

        match &result {
            Ok(tx) => {
                eprintln!("[oubli] auto_rollover: submitted tx {tx}, waiting for confirmation");
                // Wait for confirmation, then re-fetch so cipher_balance is correct
                if let Err(e) = self.wait_for_tx(tx).await {
                    eprintln!("[oubli] auto_rollover: wait failed: {e}");
                }
                if let (Some(rpc), Some(ref mut acct)) = (&self.rpc, &mut self.active_account) {
                    let _ = fetch_and_decrypt_balance(rpc, acct).await;
                }
            }
            Err(e) => eprintln!("[oubli] auto_rollover: failed (will retry): {e}"),
        }
    }

    // ── Seed phrase retrieval ────────────────────────────────

    pub fn get_mnemonic(&self) -> Result<String, WalletError> {
        let salt = self
            .storage
            .secure_load(KEK_SALT_KEY)
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?
            .ok_or_else(|| WalletError::NoActiveAccount)?;

        let blob_bytes = self
            .storage
            .secure_load(SEED_BLOB_KEY)
            .map_err(|e| WalletError::Store(oubli_store::StoreError::Platform(e)))?
            .ok_or_else(|| WalletError::NoActiveAccount)?;

        let kek = KekDerivation::derive_kek(&salt)?;
        let blob = EncryptedBlob::from_bytes(&blob_bytes)?;
        let seed_bytes = BlobManager::unwrap(&kek, &blob, APP_ID)?;

        String::from_utf8(seed_bytes).map_err(|e| WalletError::Kms(e.to_string()))
    }

    pub fn chain_id(&self) -> &str {
        &self.config.chain_id
    }

    // ── Internal helpers ─────────────────────────────────────

    fn init_rpc(&mut self) -> Result<(), WalletError> {
        if self.rpc.is_none() {
            self.rpc = Some(RpcClient::new(&self.config)?);
        }
        Ok(())
    }

    fn format_balance(&self) -> (String, String) {
        match &self.active_account {
            Some(acct) => {
                let balance_sats = format_sats_display(&tongo_units_to_sats(acct.balance as u64));
                let pending_sats = format_sats_display(&tongo_units_to_sats(acct.pending as u64));
                (balance_sats, pending_sats)
            }
            None => ("0".into(), "0".into()),
        }
    }

    // ── Operation dispatch (avoids borrow issues at bridge) ──

    /// Re-sync balance and cipher_balance from chain right before generating a proof.
    /// This ensures the cipher_balance matches the on-chain state, which is critical
    /// for ZK proof validity (especially after auto-fund or auto-rollover).
    async fn sync_balance_for_proof(&mut self) -> Result<(), WalletError> {
        let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
        let acct = self
            .active_account
            .as_mut()
            .ok_or(WalletError::NoActiveAccount)?;
        fetch_and_decrypt_balance(rpc, acct).await
    }

    pub async fn handle_fund(&mut self, amount_sats: &str) -> Result<String, WalletError> {
        self.require_t2()?;
        self.set_processing("fund");
        // Re-sync cipher_balance from chain before generating proof
        self.sync_balance_for_proof().await?;
        let config = self.config.clone();
        let result = {
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            crate::operations::execute_fund(acct, amount_sats, &config, rpc, submitter).await
        };
        match &result {
            Ok(_) => self.set_ready(),
            Err(e) => self.set_error(e.to_string()),
        }
        result
    }

    pub async fn handle_rollover_op(&mut self) -> Result<String, WalletError> {
        self.require_t2()?;
        self.set_processing("rollover");
        let config = self.config.clone();
        let result = {
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            crate::operations::execute_rollover(acct, &config, rpc, submitter).await
        };
        match &result {
            Ok(_) => self.set_ready(),
            Err(e) => self.set_error(e.to_string()),
        }
        result
    }

    /// Unified send: routes to transfer (Tongo pubkey) or withdraw (Starknet address).
    pub async fn handle_send(
        &mut self,
        amount_sats: &str,
        recipient: &str,
    ) -> Result<String, WalletError> {
        let stripped = recipient.strip_prefix("0x").unwrap_or(recipient);
        if stripped.len() > 64 {
            // Tongo public key (128 hex chars) → private transfer
            self.handle_transfer_op(amount_sats, recipient).await
        } else {
            // Starknet address → withdraw
            self.handle_withdraw_op(amount_sats, recipient).await
        }
    }

    pub async fn handle_transfer_op(
        &mut self,
        amount_sats: &str,
        recipient: &str,
    ) -> Result<String, WalletError> {
        self.require_t2()?;
        self.set_processing("transfer");
        // Re-sync cipher_balance from chain before generating proof
        self.sync_balance_for_proof().await?;
        let config = self.config.clone();
        let result = {
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            crate::operations::execute_transfer(
                acct,
                amount_sats,
                recipient,
                &config,
                rpc,
                submitter,
            )
            .await
        };
        match &result {
            Ok(ref tx_hash) => {
                // Store amount + recipient locally so it shows even if on-chain hint is missing.
                self.store_transfer_meta(tx_hash, amount_sats, Some(recipient));
                self.add_optimistic_activity("TransferOut", amount_sats, tx_hash);
                self.set_ready();
            }
            Err(e) => self.set_error(e.to_string()),
        }
        result
    }

    pub async fn handle_withdraw_op(
        &mut self,
        amount_sats: &str,
        recipient: &str,
    ) -> Result<String, WalletError> {
        self.require_t2()?;
        self.set_processing("withdraw");
        // Re-sync cipher_balance from chain before generating proof
        self.sync_balance_for_proof().await?;
        let config = self.config.clone();
        let result = {
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            crate::operations::execute_withdraw(
                acct,
                amount_sats,
                recipient,
                &config,
                rpc,
                submitter,
            )
            .await
        };
        match &result {
            Ok(ref tx_hash) => {
                // Include fee in the displayed amount so the user sees the total deducted.
                // Self-withdraws don't incur a fee.
                let display_sats = if let (Ok(sats), Some(acct)) =
                    (amount_sats.parse::<u64>(), self.active_account.as_ref())
                {
                    let is_self = Felt::from_hex(recipient)
                        .map(|r| r == acct.starknet_address)
                        .unwrap_or(false);
                    if is_self {
                        amount_sats.to_string()
                    } else {
                        let fee = crate::denomination::calculate_fee_sats(sats, config.fee_percent);
                        (sats + fee).to_string()
                    }
                } else {
                    amount_sats.to_string()
                };
                self.store_transfer_meta(tx_hash, &display_sats, Some(recipient));
                self.add_optimistic_activity("Withdraw", &display_sats, tx_hash);
                self.set_ready();
            }
            Err(e) => self.set_error(e.to_string()),
        }
        result
    }

    pub async fn send_token(
        &mut self,
        to_address: &Felt,
        amount: u128,
    ) -> Result<String, WalletError> {
        self.require_t2()?;
        let config = self.config.clone();
        let erc20_addr = Felt::from_hex(&config.token_contract)
            .map_err(|e| WalletError::Kms(format!("invalid token contract: {e}")))?;

        // Build ERC20 transfer(recipient, amount_u256) call directly.
        // Note: build_erc20_approve builds an "approve" call, not a "transfer".
        let to_rs = krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(
            &to_address.to_bytes_be(),
        );
        let erc20_rs = krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(
            &erc20_addr.to_bytes_be(),
        );
        let amount_low = krusty_kms_client::starknet_rust::core::types::Felt::from(amount);
        let amount_high = krusty_kms_client::starknet_rust::core::types::Felt::ZERO;
        let transfer_selector =
            krusty_kms_client::starknet_rust::core::utils::get_selector_from_name("transfer")
                .map_err(|e| WalletError::Kms(format!("selector error: {e}")))?;
        let transfer_call = krusty_kms_client::starknet_rust::core::types::Call {
            to: erc20_rs,
            selector: transfer_selector,
            calldata: vec![to_rs, amount_low, amount_high],
        };

        let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
        let acct = self
            .active_account
            .as_ref()
            .ok_or(WalletError::NoActiveAccount)?;
        self.submitter.ensure_deployed(acct, &config, rpc).await?;

        let acct = self
            .active_account
            .as_ref()
            .ok_or(WalletError::NoActiveAccount)?;
        let tx_hash = self.submitter.submit(acct, vec![transfer_call]).await?;
        Ok(tx_hash)
    }

    pub async fn handle_ragequit_op(&mut self, recipient: &str) -> Result<String, WalletError> {
        self.require_t2()?;
        self.set_processing("ragequit");
        // Re-sync cipher_balance from chain before generating proof
        self.sync_balance_for_proof().await?;
        let config = self.config.clone();
        let result = {
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            crate::operations::execute_ragequit(acct, recipient, &config, rpc, submitter).await
        };
        match &result {
            Ok(_) => self.set_ready(),
            Err(e) => self.set_error(e.to_string()),
        }
        result
    }
}

fn normalize_activity_events(events: &mut [ActivityEvent]) {
    for event in events {
        event.normalize();
    }
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ── Balance fetch helper (free fn to avoid borrow issues) ────

async fn fetch_and_decrypt_balance(
    rpc: &RpcClient,
    acct: &mut ActiveAccount,
) -> Result<(), WalletError> {
    let pk = &acct.tongo_account.keypair.public_key;
    let state = rpc
        .contract()
        .get_state(pk)
        .await
        .map_err(|e| WalletError::Rpc(e.to_string()))?;

    let sk = &acct.tongo_account.keypair.private_key;
    let balance = krusty_kms_client::decrypt_cipher_balance(
        &sk.expose_secret(),
        &krusty_kms_client::CipherBalance {
            l: state.balance.l.clone(),
            r: state.balance.r.clone(),
        },
    )
    .map_err(|e| WalletError::Rpc(e.to_string()))?;
    let pending = krusty_kms_client::decrypt_cipher_balance(
        &sk.expose_secret(),
        &krusty_kms_client::CipherBalance {
            l: state.pending.l.clone(),
            r: state.pending.r.clone(),
        },
    )
    .map_err(|e| WalletError::Rpc(e.to_string()))?;

    acct.balance = balance;
    acct.pending = pending;
    acct.nonce = state.nonce;
    acct.cipher_balance = Some(krusty_kms_client::CipherBalance {
        l: state.balance.l,
        r: state.balance.r,
    });

    // Sync TongoAccount internal state so KMS SDK balance checks are correct
    let nonce_bytes = state.nonce.to_bytes_be();
    let nonce_u64 = u64::from_be_bytes(nonce_bytes[24..32].try_into().unwrap_or([0u8; 8]));
    acct.tongo_account
        .update_state(krusty_kms_common::AccountState {
            balance,
            pending_balance: pending,
            nonce: nonce_u64,
        });

    // Fetch auditor key from the contract (required for audited operations)
    match rpc.contract().auditor_key().await {
        Ok(key) => {
            acct.auditor_key = key;
        }
        Err(_) => {
            // Non-fatal: contract may not have an auditor configured
        }
    }

    Ok(())
}

// ── BTC ↔ WBTC Swap operations ──────────────────────────────

impl WalletCore {
    /// Ensure the swap engine is initialized. Lazy-init on first call.
    async fn ensure_swap_engine(&mut self) -> Result<(), WalletError> {
        if self.swap_engine.is_some() {
            return Ok(());
        }

        let acct = self
            .active_account
            .as_ref()
            .ok_or(WalletError::NoActiveAccount)?;

        let engine = crate::swap::create_swap_engine(
            Arc::clone(&self.storage),
            &acct.starknet_address,
            &acct.starknet_public_key,
            &acct.starknet_private_key,
            &self.config.chain_id,
            &self.config.rpc_url,
            &self.config.account_class_hash,
            &self.config.paymaster_url,
            self.config.paymaster_api_key.as_deref(),
        )
        .await?;

        self.swap_engine = Some(engine);
        Ok(())
    }

    /// Create a BTC → WBTC on-chain swap. Returns a quote with BTC address.
    pub async fn handle_swap_btc_to_wbtc(
        &mut self,
        amount_sats: u64,
        exact_in: bool,
    ) -> Result<oubli_swap::types::SwapQuote, WalletError> {
        self.require_t2()?;
        self.ensure_swap_engine().await?;
        self.swap_engine
            .as_ref()
            .unwrap()
            .create_btc_to_wbtc(amount_sats, exact_in)
            .await
            .map_err(crate::swap::swap_err)
    }

    /// Create a WBTC → BTC off-ramp swap.
    pub async fn handle_swap_wbtc_to_btc(
        &mut self,
        amount_sats: u64,
        btc_address: &str,
        exact_in: bool,
    ) -> Result<oubli_swap::types::SwapQuote, WalletError> {
        self.require_t2()?;
        self.ensure_swap_engine().await?;
        self.swap_engine
            .as_ref()
            .unwrap()
            .create_wbtc_to_btc(amount_sats, btc_address, exact_in)
            .await
            .map_err(crate::swap::swap_err)
    }

    /// Create a Lightning BTC → WBTC swap. Returns a quote with LN invoice.
    /// Syncs balance first so the account is initialised and any public tokens
    /// are swept (which also deploys the account if needed).
    /// If the account is not deployed, deploys it via the paymaster first.
    pub async fn handle_swap_ln_to_wbtc(
        &mut self,
        amount_sats: u64,
        exact_in: bool,
    ) -> Result<oubli_swap::types::SwapQuote, WalletError> {
        self.require_t2()?;
        let _ = self.handle_refresh_balance().await;

        // Deploy the account via paymaster if not yet on-chain.
        self.ensure_account_deployed().await?;

        self.ensure_swap_engine().await?;
        self.swap_engine
            .as_ref()
            .unwrap()
            .create_ln_to_wbtc(amount_sats, exact_in)
            .await
            .map_err(crate::swap::swap_err)
    }

    /// Execute a pending swap (sign and submit Starknet txs).
    pub async fn handle_swap_execute(&mut self, swap_id: &str) -> Result<(), WalletError> {
        self.require_t2()?;
        self.ensure_swap_engine().await?;
        let engine = self
            .swap_engine
            .as_ref()
            .ok_or(WalletError::Network("swap engine not initialized".into()))?;
        engine
            .execute_swap(swap_id)
            .await
            .map_err(crate::swap::swap_err)
    }

    /// Wait for an incoming Lightning payment and claim WBTC.
    /// Used after `handle_swap_ln_to_wbtc` — blocks until the payer pays the
    /// LN invoice, then claims WBTC from the LP escrow.
    pub async fn handle_receive_lightning_wait(
        &mut self,
        swap_id: &str,
    ) -> Result<(), WalletError> {
        self.require_t2()?;
        self.ensure_swap_engine().await?;
        let engine = self
            .swap_engine
            .as_ref()
            .ok_or(WalletError::Network("swap engine not initialized".into()))?;
        engine
            .wait_for_incoming_swap(swap_id)
            .await
            .map_err(crate::swap::swap_err)?;
        // Refresh balance so auto-fund picks up the received WBTC
        if let (Some(rpc), Some(ref mut acct)) = (&self.rpc, &mut self.active_account) {
            let _ = fetch_and_decrypt_balance(rpc, acct).await;
        }
        Ok(())
    }

    /// Get the status of a swap.
    pub async fn handle_swap_status(
        &mut self,
        swap_id: &str,
    ) -> Result<oubli_swap::types::SwapStatus, WalletError> {
        self.ensure_swap_engine().await?;
        let engine = self
            .swap_engine
            .as_ref()
            .ok_or(WalletError::Network("swap engine not initialized".into()))?;
        engine
            .get_swap_status(swap_id)
            .await
            .map_err(crate::swap::swap_err)
    }

    /// Get all active/pending swaps.
    pub async fn handle_swap_list(
        &mut self,
    ) -> Result<Vec<oubli_swap::types::SwapSummary>, WalletError> {
        self.ensure_swap_engine().await?;
        let engine = self
            .swap_engine
            .as_ref()
            .ok_or(WalletError::Network("swap engine not initialized".into()))?;
        engine.get_all_swaps().await.map_err(crate::swap::swap_err)
    }

    /// Get swap limits for a direction.
    pub async fn handle_swap_limits(
        &mut self,
        direction: &str,
    ) -> Result<oubli_swap::types::SwapLimits, WalletError> {
        self.ensure_swap_engine().await?;
        let dir = match direction {
            "wbtc_to_btc" => oubli_swap::types::SwapDirection::WbtcToBtc,
            "ln_to_wbtc" => oubli_swap::types::SwapDirection::LnToWbtc,
            _ => oubli_swap::types::SwapDirection::BtcToWbtc,
        };
        self.swap_engine
            .as_ref()
            .unwrap()
            .get_swap_limits(dir)
            .await
            .map_err(crate::swap::swap_err)
    }

    /// Pay a Lightning invoice by scanning a BOLT11 QR code.
    ///
    /// Orchestrates the full flow:
    /// 1. Create WBTC→BTCLN swap quote (negotiate with LP, determine WBTC needed)
    /// 2. Withdraw the needed WBTC from Tongo to own Starknet address
    /// 3. Wait for the Tongo withdraw tx to confirm
    /// 4. Execute the swap (approve WBTC + lock in escrow via paymaster)
    /// 5. LP pays the Lightning invoice
    pub async fn handle_pay_lightning(&mut self, bolt11: &str) -> Result<String, WalletError> {
        self.require_t2()?;
        self.set_processing("ln: creating swap...");

        // 1. Init swap engine and create quote
        self.ensure_swap_engine().await?;
        let quote = self
            .swap_engine
            .as_ref()
            .unwrap()
            .create_wbtc_to_btc_ln(bolt11)
            .await
            .map_err(crate::swap::swap_err)?;

        let swap_id = quote.swap_id.clone();
        eprintln!(
            "[oubli] pay_lightning: quote created, swap_id={}, input={}, output={}",
            swap_id, quote.input_amount, quote.output_amount
        );

        // 2. Withdraw the needed WBTC from Tongo to own Starknet address.
        //    The SDK returns amounts in BTC (e.g. "0.00003037"), convert to sats.
        //    Round up to the next multiple of 10 sats (1 tongo unit = 10 sats).
        self.set_processing("ln: withdrawing WBTC...");
        let input_sats: u64 = btc_str_to_sats(&quote.input_amount).ok_or_else(|| {
            WalletError::Denomination(format!("invalid swap input amount: {}", quote.input_amount))
        })?;
        let rounded_sats = ((input_sats + 9) / 10) * 10;
        let input_amount_str = rounded_sats.to_string();
        eprintln!(
            "[oubli] pay_lightning: input_sats={}, rounded_withdraw={}",
            input_sats, rounded_sats
        );

        let own_address = self
            .starknet_address_hex
            .clone()
            .ok_or(WalletError::NoActiveAccount)?;

        // Re-sync balance before generating the withdraw proof
        self.sync_balance_for_proof().await?;

        let withdraw_result = {
            let config = self.config.clone();
            let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
            let submitter = &*self.submitter;
            let acct = self
                .active_account
                .as_mut()
                .ok_or(WalletError::NoActiveAccount)?;
            crate::operations::execute_withdraw(
                acct,
                &input_amount_str,
                &own_address,
                &config,
                rpc,
                submitter,
            )
            .await
        };

        let withdraw_tx = match withdraw_result {
            Ok(tx) => tx,
            Err(e) => {
                self.set_error(e.to_string());
                return Err(e);
            }
        };

        eprintln!(
            "[oubli] pay_lightning: Tongo withdraw tx submitted: {}",
            withdraw_tx
        );

        // 3. Wait for the Tongo withdraw tx to confirm (~2 min).
        //    Use a longer timeout than the default wait_for_tx (30s isn't enough).
        self.set_processing("ln: waiting for confirmation...");
        let mut confirmed = false;
        for i in 0..90 {
            {
                let rpc = self.rpc.as_ref().ok_or(WalletError::NoActiveAccount)?;
                if rpc.is_tx_confirmed(&withdraw_tx).await? {
                    confirmed = true;
                    break;
                }
            }
            if i % 15 == 14 {
                self.set_processing(&format!("ln: confirming... ({}s)", (i + 1) * 2));
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        if !confirmed {
            let err = WalletError::Rpc(format!(
                "Tongo withdraw tx {} not confirmed after 3 min",
                withdraw_tx
            ));
            self.set_error(err.to_string());
            return Err(err);
        }

        eprintln!("[oubli] pay_lightning: withdraw confirmed, executing swap");

        // 4. Execute the swap (approve WBTC + escrow via paymaster)
        self.set_processing("ln: executing swap...");
        let exec_result = self
            .swap_engine
            .as_ref()
            .unwrap()
            .execute_swap(&swap_id)
            .await
            .map_err(crate::swap::swap_err);

        match exec_result {
            Ok(()) => {
                eprintln!("[oubli] pay_lightning: swap executed successfully");
                // Refresh balance after the operation
                if let (Some(rpc), Some(ref mut acct)) = (&self.rpc, &mut self.active_account) {
                    let _ = fetch_and_decrypt_balance(rpc, acct).await;
                }
                self.set_ready();
                Ok(swap_id)
            }
            Err(e) => {
                self.set_error(e.to_string());
                Err(e)
            }
        }
    }
}

// ── BTC price helper ─────────────────────────────────────────

/// Fetch BTC/USD spot price from CoinGecko.
async fn fetch_btc_price() -> Option<f64> {
    let resp = reqwest::Client::new()
        .get("https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("bitcoin")?.get("usd")?.as_f64()
}

// ── Amount conversion helper ─────────────────────────────────

/// Convert a BTC decimal string (e.g. "0.00003037") or integer sats string to sats.
fn btc_str_to_sats(s: &str) -> Option<u64> {
    // Try parsing as integer first (already sats)
    if let Ok(sats) = s.parse::<u64>() {
        return Some(sats);
    }
    // Parse as decimal BTC: multiply by 1e8 using string math to avoid float imprecision
    let s = s.trim();
    let (whole, frac) = if let Some(dot) = s.find('.') {
        (&s[..dot], &s[dot + 1..])
    } else {
        return None;
    };
    let whole: u64 = whole.parse().ok()?;
    // Pad or truncate fraction to exactly 8 digits
    let frac_padded = format!("{:0<8}", frac);
    let frac_8 = &frac_padded[..8];
    let frac_sats: u64 = frac_8.parse().ok()?;
    Some(whole * 100_000_000 + frac_sats)
}

// ── Key derivation helper ────────────────────────────────────

fn derive_active_account(
    mnemonic: &str,
    config: &NetworkConfig,
) -> Result<ActiveAccount, WalletError> {
    let tongo_addr = Felt::from_hex(&config.tongo_contract)
        .map_err(|e| WalletError::Kms(format!("invalid tongo contract address: {e}")))?;

    let tongo_account =
        krusty_kms_sdk::TongoAccount::from_mnemonic(mnemonic, 0, 0, tongo_addr, None)
            .map_err(|e| WalletError::Kms(e.to_string()))?;

    let starknet_sk = krusty_kms::derive_private_key_with_coin_type(mnemonic, 0, 0, 9004, None)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    // Derive starknet public key (x-coordinate) from signing key
    let sk_rs = krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(
        &starknet_sk.to_bytes_be(),
    );
    let signing_key =
        krusty_kms_client::starknet_rust::signers::SigningKey::from_secret_scalar(sk_rs);
    let starknet_pub_key_rs = signing_key.verifying_key().scalar();
    let starknet_pub_key = Felt::from_bytes_be(&starknet_pub_key_rs.to_bytes_be());

    let class_hash = Felt::from_hex(&config.account_class_hash)
        .map_err(|e| WalletError::Kms(format!("invalid account class hash: {e}")))?;

    let class_hash_rs = krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(
        &class_hash.to_bytes_be(),
    );
    let constructor_calldata = if config.account_class_hash == DEVNET_CUSTOM_ACCOUNT_CLASS_HASH {
        // Devnet's built-in "Custom" account class takes a single `public_key`.
        vec![starknet_pub_key_rs]
    } else {
        // ArgentX v0.4 constructor takes:
        //   owner:    CairoCustomEnum { Starknet: { pubkey } } → serializes as [0, pubkey]
        //   guardian: CairoOption::None                         → serializes as [1]
        vec![
            krusty_kms_client::starknet_rust::core::types::Felt::ZERO, // owner enum variant: 0 = Starknet
            starknet_pub_key_rs,                                       // owner pubkey
            krusty_kms_client::starknet_rust::core::types::Felt::ONE, // guardian: CairoOption::None = 1
        ]
    };
    let addr_rs = krusty_kms_client::starknet_rust::core::utils::get_contract_address(
        krusty_kms_client::starknet_rust::core::types::Felt::ZERO, // salt
        class_hash_rs,
        &constructor_calldata,
        krusty_kms_client::starknet_rust::core::types::Felt::ZERO, // deployer
    );
    let starknet_address = Felt::from_bytes_be(&addr_rs.to_bytes_be());

    // Tongo public key hex (full point) for display
    let owner_pub_hex = tongo_account
        .owner_public_key_hex()
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    Ok(ActiveAccount {
        tongo_account,
        starknet_private_key: starknet_sk,
        starknet_address,
        starknet_public_key: starknet_pub_key,
        owner_public_key_hex: owner_pub_hex,
        balance: 0,
        pending: 0,
        nonce: Felt::ZERO,
        cipher_balance: None,
        auditor_key: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oubli_store::MockPlatformStorage;
    use std::collections::HashMap;

    fn test_config() -> NetworkConfig {
        crate::networks::sepolia::config()
    }

    fn test_mnemonic() -> &'static str {
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    }

    fn store_seed_blob(storage: &MockPlatformStorage, mnemonic: &str) {
        let salt = storage.generate_hardware_salt().unwrap();
        storage.secure_store(KEK_SALT_KEY, &salt).unwrap();
        let kek = KekDerivation::derive_kek(&salt).unwrap();
        let blob = BlobManager::wrap(&kek, mnemonic.as_bytes(), APP_ID).unwrap();
        storage
            .secure_store(SEED_BLOB_KEY, &blob.to_bytes())
            .unwrap();
    }

    #[test]
    fn test_new_wallet_starts_onboarding() {
        let storage = Box::new(MockPlatformStorage::new());
        let core = WalletCore::new(storage, test_config());
        assert_eq!(*core.state(), WalletState::Onboarding);
        assert_eq!(core.auth_tier(), AuthTier::T0Locked);
        assert!(core.active_account().is_none());
    }

    #[test]
    fn test_returning_user_starts_locked() {
        let storage = MockPlatformStorage::new();
        storage.secure_store(PUBKEY_KEY, b"0xdeadbeef").unwrap();
        storage.secure_store(STARKNET_ADDR_KEY, b"0xcafe").unwrap();
        let core = WalletCore::new(Box::new(storage), test_config());
        assert_eq!(*core.state(), WalletState::Locked);
    }

    #[test]
    fn test_lock_clears_active_account() {
        let storage = Box::new(MockPlatformStorage::new());
        let mut core = WalletCore::new(storage, test_config());
        // Simulate having an active account by setting wallet state
        core.wallet_state = WalletState::Ready {
            address: "0x123".into(),
            balance_sats: "0".into(),
            pending_sats: "0".into(),
        };
        core.handle_lock();
        assert!(core.active_account().is_none());
        assert_eq!(*core.state(), WalletState::Onboarding);
        assert_eq!(core.auth_tier(), AuthTier::T0Locked);
    }

    #[test]
    fn test_lock_keeps_returning_user_locked() {
        let storage = MockPlatformStorage::new();
        storage.secure_store(PUBKEY_KEY, b"0xdeadbeef").unwrap();
        storage.secure_store(STARKNET_ADDR_KEY, b"0xcafe").unwrap();
        let mut core = WalletCore::new(Box::new(storage), test_config());

        core.wallet_state = WalletState::Ready {
            address: "0xcafe".into(),
            balance_sats: "5".into(),
            pending_sats: "1".into(),
        };
        core.handle_lock();

        assert_eq!(*core.state(), WalletState::Locked);
        assert!(core.active_account().is_none());
        assert_eq!(core.auth_tier(), AuthTier::T0Locked);
    }

    #[test]
    fn test_auth_tier_enforcement() {
        let storage = Box::new(MockPlatformStorage::new());
        let core = WalletCore::new(storage, test_config());
        // At T0, require_t2 should fail
        assert!(core.require_t2().is_err());
    }

    #[test]
    fn test_require_t2_succeeds_with_loaded_account() {
        let storage = Box::new(MockPlatformStorage::new());
        let mut core = WalletCore::new(storage, test_config());
        core.active_account = Some(derive_active_account(test_mnemonic(), &test_config()).unwrap());
        core.auth_state.apply(AuthAction::BiometricSuccess);

        assert!(core.require_t2().is_ok());
    }

    #[test]
    fn test_handle_background_drops_tier() {
        let storage = MockPlatformStorage::new();
        storage.secure_store(PUBKEY_KEY, b"0xdeadbeef").unwrap();
        storage.secure_store(STARKNET_ADDR_KEY, b"0xcafe").unwrap();
        let mut core = WalletCore::new(Box::new(storage), test_config());
        // Simulate T2
        core.auth_state.apply(AuthAction::BiometricSuccess);
        assert_eq!(core.auth_tier(), AuthTier::T2Transact);

        core.handle_background();
        // Background drops T2 → T0
        assert_eq!(core.auth_tier(), AuthTier::T0Locked);
        assert!(core.active_account().is_none());
    }

    #[test]
    fn test_get_mnemonic_round_trip_from_stored_blob() {
        let storage = MockPlatformStorage::new();
        store_seed_blob(&storage, test_mnemonic());
        let core = WalletCore::new(Box::new(storage), test_config());

        assert_eq!(core.get_mnemonic().unwrap(), test_mnemonic());
    }

    #[test]
    fn test_get_mnemonic_without_seed_returns_no_active_account() {
        let core = WalletCore::new(Box::new(MockPlatformStorage::new()), test_config());
        assert!(matches!(
            core.get_mnemonic(),
            Err(WalletError::NoActiveAccount)
        ));
    }

    #[test]
    fn test_load_transfer_metas_migrates_legacy_amount_map() {
        let storage = MockPlatformStorage::new();
        let legacy = HashMap::from([("0xtx".to_string(), "250".to_string())]);
        storage
            .secure_store(TRANSFER_AMOUNTS_KEY, &serde_json::to_vec(&legacy).unwrap())
            .unwrap();

        let core = WalletCore::new(Box::new(storage), test_config());
        let metas = core.load_transfer_metas();

        assert_eq!(metas["0xtx"].amount_sats, "250");
        assert_eq!(metas["0xtx"].recipient, None);

        let migrated_bytes = core
            .storage
            .secure_load(TRANSFER_AMOUNTS_KEY)
            .unwrap()
            .unwrap();
        let migrated: HashMap<String, TransferMeta> =
            serde_json::from_slice(&migrated_bytes).unwrap();
        assert_eq!(migrated["0xtx"].amount_sats, "250");
        assert_eq!(migrated["0xtx"].recipient, None);
    }

    #[test]
    fn test_get_cached_activity_normalizes_and_backfills_transfer_amounts() {
        let storage = MockPlatformStorage::new();
        let core = WalletCore::new(Box::new(storage), test_config());

        core.store_transfer_meta("0xtx", "1000", Some("0xrecipient"));
        let cached = vec![
            ActivityEvent {
                event_type: "TransferOut".into(),
                amount_sats: None,
                tx_hash: "0xtx".into(),
                block_number: 0,
                timestamp_secs: None,
                status: ActivityStatus::Unknown,
            },
            ActivityEvent {
                event_type: "TransferIn".into(),
                amount_sats: Some("200".into()),
                tx_hash: "0xconfirmed".into(),
                block_number: 42,
                timestamp_secs: Some(123),
                status: ActivityStatus::Unknown,
            },
        ];
        core.storage
            .secure_store(ACTIVITY_CACHE_KEY, &serde_json::to_vec(&cached).unwrap())
            .unwrap();

        let loaded = core.get_cached_activity();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].status, ActivityStatus::Pending);
        assert_eq!(loaded[0].amount_sats.as_deref(), Some("1000"));
        assert_eq!(loaded[1].status, ActivityStatus::Confirmed);
        assert_eq!(
            core.get_transfer_recipient("0xtx").as_deref(),
            Some("0xrecipient")
        );
    }

    #[test]
    fn test_add_optimistic_activity_persists_pending_event_and_truncates() {
        let storage = MockPlatformStorage::new();
        let core = WalletCore::new(Box::new(storage), test_config());

        let cached: Vec<ActivityEvent> = (0..20)
            .map(|idx| ActivityEvent {
                event_type: "TransferIn".into(),
                amount_sats: Some(idx.to_string()),
                tx_hash: format!("0x{idx:02x}"),
                block_number: 10 + idx,
                timestamp_secs: Some(1_700_000_000 + idx),
                status: ActivityStatus::Confirmed,
            })
            .collect();
        core.storage
            .secure_store(ACTIVITY_CACHE_KEY, &serde_json::to_vec(&cached).unwrap())
            .unwrap();

        core.add_optimistic_activity("TransferOut", "321", "0xnew");
        let loaded = core.get_cached_activity();

        assert_eq!(loaded.len(), 20);
        assert_eq!(loaded[0].tx_hash, "0xnew");
        assert_eq!(loaded[0].status, ActivityStatus::Pending);
        assert_eq!(loaded[0].amount_sats.as_deref(), Some("321"));
        assert!(loaded[0].timestamp_secs.is_some());
        assert!(!loaded.iter().any(|event| event.tx_hash == "0x13"));
    }

    #[tokio::test]
    async fn test_handle_unlock_biometric_rejects_failed_authentication() {
        let storage = MockPlatformStorage::new().with_biometric(true, false);
        storage.secure_store(PUBKEY_KEY, b"0xdeadbeef").unwrap();
        storage.secure_store(STARKNET_ADDR_KEY, b"0xcafe").unwrap();
        let mut core = WalletCore::new(Box::new(storage), test_config());

        let err = core.handle_unlock_biometric().await.unwrap_err();
        match err {
            WalletError::InvalidState { expected, got } => {
                assert_eq!(expected, "biometric success");
                assert_eq!(got, "biometric authentication failed");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(core.auth_tier(), AuthTier::T0Locked);
    }

    #[tokio::test]
    async fn test_handle_unlock_biometric_rejects_when_not_locked() {
        let storage = MockPlatformStorage::new().with_biometric(true, true);
        storage.secure_store(PUBKEY_KEY, b"0xdeadbeef").unwrap();
        storage.secure_store(STARKNET_ADDR_KEY, b"0xcafe").unwrap();
        let mut core = WalletCore::new(Box::new(storage), test_config());
        core.auth_state.apply(AuthAction::BiometricSuccess);

        let err = core.handle_unlock_biometric().await.unwrap_err();
        match err {
            WalletError::InvalidState { expected, got } => {
                assert_eq!(expected, "T0Locked");
                assert_eq!(got, "T2Transact");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(core.auth_tier(), AuthTier::T2Transact);
    }
}

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod telemetry;

use oubli_backup::{SeedDisplayFlow, VerificationPrompt};
use oubli_wallet::{NetworkConfig, WalletCore, WalletState};

// ── FFI error ────────────────────────────────────────────────

/// Flat error type for FFI — variant carries the category, message is in Display.
/// UniFFI flat [Error] enums must have unit variants.
#[derive(Debug)]
pub enum OubliError {
    Auth,
    Store,
    Backup,
    Kms,
    Rpc,
    Paymaster,
    InvalidState,
    NoActiveAccount,
    Denomination,
    InsufficientBalance,
    Network,
}

// We need a per-thread message since flat enums can't carry data.
// Use a simple approach: store the message in thread-local storage.
thread_local! {
    static LAST_ERROR_MSG: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn set_error_msg(msg: String) {
    LAST_ERROR_MSG.with(|m| *m.borrow_mut() = msg);
}

fn get_error_msg() -> String {
    LAST_ERROR_MSG.with(|m| m.borrow().clone())
}

impl fmt::Display for OubliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = get_error_msg();
        if msg.is_empty() {
            write!(f, "{:?}", self)
        } else {
            write!(f, "{msg}")
        }
    }
}

impl std::error::Error for OubliError {}

fn auth_err(msg: impl Into<String>) -> OubliError {
    set_error_msg(msg.into());
    OubliError::Auth
}

fn krusty_kms_err(msg: impl Into<String>) -> OubliError {
    set_error_msg(msg.into());
    OubliError::Kms
}

fn backup_err(msg: impl Into<String>) -> OubliError {
    set_error_msg(msg.into());
    OubliError::Backup
}

impl From<oubli_auth::AuthError> for OubliError {
    fn from(e: oubli_auth::AuthError) -> Self {
        auth_err(e.to_string())
    }
}

impl From<oubli_store::StoreError> for OubliError {
    fn from(e: oubli_store::StoreError) -> Self {
        set_error_msg(e.to_string());
        OubliError::Store
    }
}

impl From<oubli_backup::BackupError> for OubliError {
    fn from(e: oubli_backup::BackupError) -> Self {
        backup_err(e.to_string())
    }
}

impl From<oubli_wallet::WalletError> for OubliError {
    fn from(e: oubli_wallet::WalletError) -> Self {
        let msg = e.to_string();
        let variant = match e {
            oubli_wallet::WalletError::Auth(_) => OubliError::Auth,
            oubli_wallet::WalletError::Store(_) => OubliError::Store,
            oubli_wallet::WalletError::Backup(_) => OubliError::Backup,
            oubli_wallet::WalletError::Kms(_) => OubliError::Kms,
            oubli_wallet::WalletError::Rpc(_) => OubliError::Rpc,
            oubli_wallet::WalletError::Paymaster(_) => OubliError::Paymaster,
            oubli_wallet::WalletError::InvalidState { .. } => OubliError::InvalidState,
            oubli_wallet::WalletError::NoActiveAccount => OubliError::NoActiveAccount,
            oubli_wallet::WalletError::Denomination(_) => OubliError::Denomination,
            oubli_wallet::WalletError::InsufficientBalance { .. } => {
                OubliError::InsufficientBalance
            }
            oubli_wallet::WalletError::Network(_) => OubliError::Network,
            oubli_wallet::WalletError::OperationInProgress => OubliError::InvalidState,
            oubli_wallet::WalletError::Signing(_) => OubliError::Kms,
            oubli_wallet::WalletError::TypedDataValidation(_) => OubliError::InvalidState,
        };
        set_error_msg(msg);
        variant
    }
}

// ── FFI state types ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum WalletStateFFI {
    Onboarding,
    Locked,
    Ready,
    Processing,
    Error,
    SeedBackup,
    Wiped,
}

#[derive(Debug, Clone)]
pub struct WalletStateInfo {
    pub state: WalletStateFFI,
    pub address: Option<String>,
    pub public_key: Option<String>,
    pub balance_sats: Option<String>,
    pub pending_sats: Option<String>,
    pub operation: Option<String>,
    pub error_message: Option<String>,
    pub auto_fund_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VerificationPromptFFI {
    pub word_number: u32,
}

#[derive(Debug, Clone)]
pub struct SeedBackupStateFFI {
    pub word_groups: Vec<Vec<String>>,
    pub prompts: Vec<VerificationPromptFFI>,
}

// ── Activity event FFI ───────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ActivityEventFFI {
    pub event_type: String,
    pub amount_sats: Option<String>,
    pub tx_hash: String,
    pub block_number: u64,
    pub timestamp_secs: Option<u64>,
    pub status: String,
    pub explorer_url: Option<String>,
}

// ── Swap FFI types ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SwapQuoteFFI {
    pub swap_id: String,
    pub input_amount: String,
    pub output_amount: String,
    pub fee: String,
    pub expiry: u64,
    pub btc_address: Option<String>,
    pub ln_invoice: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SwapStatusFFI {
    pub swap_id: String,
    pub state: String,
    pub tx_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SwapSummaryFFI {
    pub swap_id: String,
    pub state: String,
    pub input_amount: String,
    pub output_amount: String,
}

#[derive(Debug, Clone)]
pub struct SwapLimitsFFI {
    pub input_min: String,
    pub input_max: Option<String>,
    pub output_min: String,
    pub output_max: Option<String>,
}

// ── Contact FFI types ────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum AddressTypeFFI {
    Oubli,
    Starknet,
}

#[derive(Debug, Clone)]
pub struct ContactAddressFFI {
    pub address: String,
    pub address_type: AddressTypeFFI,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContactFFI {
    pub id: String,
    pub name: String,
    pub addresses: Vec<ContactAddressFFI>,
    pub notes: Option<String>,
    pub created_at: u64,
    pub last_used_at: u64,
}

impl From<oubli_wallet::Contact> for ContactFFI {
    fn from(c: oubli_wallet::Contact) -> Self {
        ContactFFI {
            id: c.id,
            name: c.name,
            addresses: c
                .addresses
                .into_iter()
                .map(|a| ContactAddressFFI {
                    address: a.address,
                    address_type: match a.address_type {
                        oubli_wallet::AddressType::Oubli => AddressTypeFFI::Oubli,
                        oubli_wallet::AddressType::Starknet => AddressTypeFFI::Starknet,
                    },
                    label: a.label,
                })
                .collect(),
            notes: c.notes,
            created_at: c.created_at,
            last_used_at: c.last_used_at,
        }
    }
}

impl From<ContactFFI> for oubli_wallet::Contact {
    fn from(c: ContactFFI) -> Self {
        oubli_wallet::Contact {
            id: c.id,
            name: c.name,
            addresses: c
                .addresses
                .into_iter()
                .map(|a| oubli_wallet::ContactAddress {
                    address: a.address,
                    address_type: match a.address_type {
                        AddressTypeFFI::Oubli => oubli_wallet::AddressType::Oubli,
                        AddressTypeFFI::Starknet => oubli_wallet::AddressType::Starknet,
                    },
                    label: a.label,
                })
                .collect(),
            notes: c.notes,
            created_at: c.created_at,
            last_used_at: c.last_used_at,
        }
    }
}

fn tx_explorer_url(chain_id: &str, tx_hash: &str) -> Option<String> {
    if tx_hash.trim().is_empty() {
        return None;
    }

    let base = match chain_id {
        "SN_MAIN" => "https://starkscan.co/tx/",
        "SN_SEPOLIA" => "https://sepolia.starkscan.co/tx/",
        _ => return None,
    };

    Some(format!("{base}{tx_hash}"))
}

// ── Platform storage callback ────────────────────────────────

pub trait PlatformStorageCallback: Send + Sync {
    fn secure_store(&self, key: String, value: Vec<u8>) -> Result<(), OubliError>;
    fn secure_load(&self, key: String) -> Result<Option<Vec<u8>>, OubliError>;
    fn secure_delete(&self, key: String) -> Result<(), OubliError>;
    fn request_biometric(&self, reason: String) -> Result<bool, OubliError>;
    fn biometric_available(&self) -> bool;
    fn generate_hardware_salt(&self) -> Result<Vec<u8>, OubliError>;
}

/// Adapter: bridge PlatformStorageCallback (UniFFI) → PlatformStorage (oubli-store trait).
struct PlatformStorageAdapter {
    callback: Box<dyn PlatformStorageCallback>,
}

impl oubli_store::PlatformStorage for PlatformStorageAdapter {
    fn secure_store(&self, key: &str, value: &[u8]) -> Result<(), String> {
        self.callback
            .secure_store(key.to_string(), value.to_vec())
            .map_err(|e| e.to_string())
    }

    fn secure_load(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        self.callback
            .secure_load(key.to_string())
            .map_err(|e| e.to_string())
    }

    fn secure_delete(&self, key: &str) -> Result<(), String> {
        self.callback
            .secure_delete(key.to_string())
            .map_err(|e| e.to_string())
    }

    fn request_biometric(&self, reason: &str) -> Result<bool, String> {
        self.callback
            .request_biometric(reason.to_string())
            .map_err(|e| e.to_string())
    }

    fn biometric_available(&self) -> bool {
        self.callback.biometric_available()
    }

    fn generate_hardware_salt(&self) -> Result<Vec<u8>, String> {
        self.callback
            .generate_hardware_salt()
            .map_err(|e| e.to_string())
    }
}

// ── Payment notification callback ───────────────────────────

/// Called from the background poll thread when a new incoming payment
/// (TransferIn or Fund) is detected on-chain.
pub trait PaymentNotificationCallback: Send + Sync {
    fn on_incoming_payment(&self, event: ActivityEventFFI);
}

// ── WalletState conversion ───────────────────────────────────

fn wallet_state_to_ffi(
    state: &WalletState,
    public_key: Option<&str>,
    auto_fund_error: Option<&str>,
) -> WalletStateInfo {
    let auto_fund_error = auto_fund_error.map(String::from);
    let public_key = public_key.map(String::from);
    match state {
        WalletState::Onboarding => WalletStateInfo {
            state: WalletStateFFI::Onboarding,
            address: None,
            public_key: None,
            balance_sats: None,
            pending_sats: None,
            operation: None,
            error_message: None,
            auto_fund_error: None,
        },
        WalletState::Locked => WalletStateInfo {
            state: WalletStateFFI::Locked,
            address: None,
            public_key: None,
            balance_sats: None,
            pending_sats: None,
            operation: None,
            error_message: None,
            auto_fund_error: None,
        },
        WalletState::Ready {
            address,
            balance_sats,
            pending_sats,
        } => WalletStateInfo {
            state: WalletStateFFI::Ready,
            address: Some(address.clone()),
            public_key,
            balance_sats: Some(balance_sats.clone()),
            pending_sats: Some(pending_sats.clone()),
            operation: None,
            error_message: None,
            auto_fund_error,
        },
        WalletState::Processing { address, operation } => WalletStateInfo {
            state: WalletStateFFI::Processing,
            address: Some(address.clone()),
            public_key,
            balance_sats: None,
            pending_sats: None,
            operation: Some(operation.clone()),
            error_message: None,
            auto_fund_error: None,
        },
        WalletState::Error { message } => WalletStateInfo {
            state: WalletStateFFI::Error,
            address: None,
            public_key: None,
            balance_sats: None,
            pending_sats: None,
            operation: None,
            error_message: Some(message.clone()),
            auto_fund_error,
        },
        WalletState::SeedBackup => WalletStateInfo {
            state: WalletStateFFI::SeedBackup,
            address: None,
            public_key: None,
            balance_sats: None,
            pending_sats: None,
            operation: None,
            error_message: None,
            auto_fund_error: None,
        },
        WalletState::Wiped => WalletStateInfo {
            state: WalletStateFFI::Wiped,
            address: None,
            public_key: None,
            balance_sats: None,
            pending_sats: None,
            operation: None,
            error_message: None,
            auto_fund_error: None,
        },
    }
}

// ── OubliWallet (FFI object) ─────────────────────────────────

/// Interval between background balance polls (seconds).
const POLL_INTERVAL_SECS: u64 = 3;

pub struct OubliWallet {
    core: Arc<Mutex<WalletCore>>,
    runtime: Arc<tokio::runtime::Runtime>,
    seed_flow: Mutex<Option<SeedDisplayFlow>>,
    seed_prompts: Mutex<Vec<VerificationPrompt>>,
    /// When true the background thread polls `handle_refresh_balance`.
    polling_active: Arc<AtomicBool>,
    /// BTC price cache independent of the core mutex to avoid deadlocks.
    /// Keyed by lowercase currency code (e.g. "usd", "eur").
    btc_price_cache: Mutex<std::collections::HashMap<String, (f64, std::time::Instant)>>,
    /// Optional callback invoked when a new incoming payment is detected.
    payment_callback: Arc<Mutex<Option<Box<dyn PaymentNotificationCallback>>>>,
}

impl OubliWallet {
    pub fn new(
        storage: Box<dyn PlatformStorageCallback>,
        rpc_url: Option<String>,
        paymaster_api_key: Option<String>,
    ) -> Result<Self, OubliError> {
        let adapter = PlatformStorageAdapter { callback: storage };
        let mut config = NetworkConfig::from_env();
        if let Some(url) = rpc_url {
            if !url.trim().is_empty() {
                if let Err(err) = config.set_rpc_url(&url) {
                    crate::bridge_warn_event!(
                        "bridge.config",
                        "invalid_rpc_override",
                        "error_kind" = telemetry::error_kind(&err)
                    );
                }
            }
        }
        if let Some(key) = paymaster_api_key {
            if !key.is_empty() {
                config.paymaster_api_key = Some(key);
            }
        }
        let core = WalletCore::new(Box::new(adapter), config);

        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|e| {
                    set_error_msg(format!("failed to create runtime: {e}"));
                    OubliError::Network
                })?,
        );

        let core = Arc::new(Mutex::new(core));
        let polling_active = Arc::new(AtomicBool::new(false));
        let payment_callback: Arc<Mutex<Option<Box<dyn PaymentNotificationCallback>>>> =
            Arc::new(Mutex::new(None));

        // Spawn background polling thread.
        // Uses try_lock so it never blocks user-initiated operations.
        // Also detects new incoming payments and fires the callback.
        {
            let poll_core = Arc::clone(&core);
            let poll_handle = runtime.handle().clone();
            let poll_flag = Arc::clone(&polling_active);
            let poll_cb = Arc::clone(&payment_callback);
            let first_poll = AtomicBool::new(true);
            std::thread::Builder::new()
                .name("oubli-poll".into())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
                    if !poll_flag.load(Ordering::Relaxed) {
                        first_poll.store(true, Ordering::Relaxed);
                        continue;
                    }
                    if let Ok(mut core) = poll_core.try_lock() {
                        // Snapshot known tx hashes before refresh
                        let known: std::collections::HashSet<String> = core
                            .get_cached_activity()
                            .iter()
                            .map(|e| e.tx_hash.clone())
                            .collect();

                        let _ = poll_handle.block_on(core.handle_refresh_balance());

                        // Skip notification on first poll (avoids false positives
                        // when the cache is empty after unlock).
                        if first_poll.swap(false, Ordering::Relaxed) {
                            continue;
                        }

                        // Fetch fresh activity and detect new incoming payments
                        if let Ok(events) = poll_handle.block_on(core.get_activity()) {
                            let chain_id = core.chain_id().to_string();
                            let new_incoming: Vec<_> = events
                                .iter()
                                .filter(|e| !known.contains(&e.tx_hash))
                                .filter(|e| e.event_type == "TransferIn" || e.event_type == "Fund")
                                .collect();

                            if !new_incoming.is_empty() {
                                if let Ok(guard) = poll_cb.lock() {
                                    if let Some(ref cb) = *guard {
                                        for event in new_incoming {
                                            let explorer_url =
                                                tx_explorer_url(&chain_id, &event.tx_hash);
                                            cb.on_incoming_payment(ActivityEventFFI {
                                                event_type: event.event_type.clone(),
                                                amount_sats: event.amount_sats.clone(),
                                                tx_hash: event.tx_hash.clone(),
                                                block_number: event.block_number,
                                                timestamp_secs: event.timestamp_secs,
                                                status: event.status.as_str().to_string(),
                                                explorer_url,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
                .ok();
        }

        Ok(Self {
            core,
            runtime,
            seed_flow: Mutex::new(None),
            seed_prompts: Mutex::new(Vec::new()),
            polling_active,
            btc_price_cache: Mutex::new(std::collections::HashMap::new()),
            payment_callback,
        })
    }

    /// Register a callback that fires when a new incoming payment is detected
    /// by the background poll thread.
    pub fn register_payment_callback(&self, callback: Box<dyn PaymentNotificationCallback>) {
        *self.payment_callback.lock().unwrap() = Some(callback);
    }

    pub fn get_state(&self) -> WalletStateInfo {
        let core = self.core.lock().unwrap();
        wallet_state_to_ffi(
            core.state(),
            core.owner_public_key(),
            core.last_auto_fund_error(),
        )
    }

    pub fn handle_complete_onboarding(&self, mnemonic: String) -> Result<(), OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_onboarding(&mnemonic))
            .map_err(OubliError::from)?;
        self.polling_active.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn handle_unlock_biometric(&self) -> Result<(), OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_unlock_biometric())
            .map_err(OubliError::from)?;
        self.polling_active.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn handle_lock(&self) -> Result<(), OubliError> {
        self.polling_active.store(false, Ordering::Relaxed);
        let mut core = self.core.lock().unwrap();
        core.handle_lock();
        Ok(())
    }

    pub fn handle_fund(&self, amount_sats: String) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_fund(&amount_sats))
            .map_err(OubliError::from)
    }

    pub fn handle_rollover(&self) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_rollover_op())
            .map_err(OubliError::from)
    }

    pub fn handle_send(
        &self,
        amount_sats: String,
        recipient: String,
    ) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_send(&amount_sats, &recipient))
            .map_err(OubliError::from)
    }

    pub fn handle_transfer(
        &self,
        amount_sats: String,
        recipient: String,
    ) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_transfer_op(&amount_sats, &recipient))
            .map_err(OubliError::from)
    }

    pub fn handle_withdraw(
        &self,
        amount_sats: String,
        recipient: String,
    ) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_withdraw_op(&amount_sats, &recipient))
            .map_err(OubliError::from)
    }

    pub fn handle_ragequit(&self, recipient: String) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_ragequit_op(&recipient))
            .map_err(OubliError::from)
    }

    pub fn handle_refresh_balance(&self) -> Result<(), OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_refresh_balance())
            .map_err(OubliError::from)
    }

    pub fn update_rpc_url(&self, url: String) {
        let mut core = self.core.lock().unwrap();
        if let Err(err) = core.update_rpc_url(url) {
            set_error_msg(err.to_string());
            crate::bridge_warn_event!(
                "bridge.config",
                "update_rpc_url_failed",
                "error_kind" = telemetry::error_kind(&err)
            );
        }
    }

    pub fn get_rpc_url(&self) -> String {
        let core = self.core.lock().unwrap();
        core.rpc_url().to_string()
    }

    pub fn handle_start_seed_backup(
        &self,
        mnemonic: String,
    ) -> Result<SeedBackupStateFFI, OubliError> {
        let flow = SeedDisplayFlow::new(&mnemonic).map_err(|e| backup_err(e.to_string()))?;

        let groups: Vec<Vec<String>> = flow.word_groups().into_iter().map(|g| g.words).collect();

        let prompts = flow.verification_prompts();
        let prompts_ffi: Vec<VerificationPromptFFI> = prompts
            .iter()
            .map(|p| VerificationPromptFFI {
                word_number: p.word_number() as u32,
            })
            .collect();

        *self.seed_prompts.lock().unwrap() = prompts;
        *self.seed_flow.lock().unwrap() = Some(flow);

        Ok(SeedBackupStateFFI {
            word_groups: groups,
            prompts: prompts_ffi,
        })
    }

    pub fn handle_verify_seed_word(
        &self,
        prompt_index: u32,
        answer: String,
    ) -> Result<bool, OubliError> {
        let prompts = self.seed_prompts.lock().unwrap();
        let prompt = prompts
            .get(prompt_index as usize)
            .ok_or_else(|| backup_err("invalid prompt index"))?;
        Ok(prompt.check(&answer))
    }

    pub fn get_mnemonic(&self) -> Result<String, OubliError> {
        let core = self.core.lock().unwrap();
        core.get_mnemonic().map_err(OubliError::from)
    }

    pub fn get_btc_price_usd(&self) -> Option<f64> {
        self.get_btc_price("usd".to_string())
    }

    pub fn get_btc_price(&self, currency: String) -> Option<f64> {
        let key = currency.to_lowercase();
        // Use bridge-level cache to avoid blocking on the core mutex.
        {
            let cache = self.btc_price_cache.lock().unwrap();
            if let Some((price, fetched_at)) = cache.get(&key) {
                if fetched_at.elapsed() < std::time::Duration::from_secs(3600) {
                    return Some(*price);
                }
            }
        }
        let price = fetch_btc_price_blocking(&key)?;
        let mut cache = self.btc_price_cache.lock().unwrap();
        cache.insert(key, (price, std::time::Instant::now()));
        Some(price)
    }

    pub fn get_activity(&self) -> Result<Vec<ActivityEventFFI>, OubliError> {
        let core = self.core.lock().unwrap();
        let chain_id = core.chain_id().to_string();
        let events = self.runtime.block_on(core.get_activity())?;
        Ok(events
            .into_iter()
            .map(|e| {
                let explorer_url = tx_explorer_url(&chain_id, &e.tx_hash);
                ActivityEventFFI {
                    event_type: e.event_type,
                    amount_sats: e.amount_sats,
                    tx_hash: e.tx_hash,
                    block_number: e.block_number,
                    timestamp_secs: e.timestamp_secs,
                    status: e.status.as_str().to_string(),
                    explorer_url,
                }
            })
            .collect())
    }

    pub fn get_cached_activity(&self) -> Vec<ActivityEventFFI> {
        let core = self.core.lock().unwrap();
        let chain_id = core.chain_id().to_string();
        core.get_cached_activity()
            .into_iter()
            .map(|e| {
                let explorer_url = tx_explorer_url(&chain_id, &e.tx_hash);
                ActivityEventFFI {
                    event_type: e.event_type,
                    amount_sats: e.amount_sats,
                    tx_hash: e.tx_hash,
                    block_number: e.block_number,
                    timestamp_secs: e.timestamp_secs,
                    status: e.status.as_str().to_string(),
                    explorer_url,
                }
            })
            .collect()
    }

    pub fn generate_mnemonic(&self) -> Result<String, OubliError> {
        krusty_kms::generate_mnemonic(12).map_err(|e| krusty_kms_err(e.to_string()))
    }

    pub fn validate_mnemonic(&self, phrase: String) -> Result<(), OubliError> {
        krusty_kms::validate_mnemonic(&phrase).map_err(|e| krusty_kms_err(e.to_string()))
    }

    // ── Swap operations ─────────────────────────────────────────

    pub fn swap_btc_to_wbtc(
        &self,
        amount_sats: u64,
        exact_in: bool,
    ) -> Result<SwapQuoteFFI, OubliError> {
        let mut core = self.core.lock().unwrap();
        let quote = self
            .runtime
            .block_on(core.handle_swap_btc_to_wbtc(amount_sats, exact_in))
            .map_err(OubliError::from)?;
        Ok(swap_quote_to_ffi(quote))
    }

    pub fn swap_wbtc_to_btc(
        &self,
        amount_sats: u64,
        btc_address: String,
        exact_in: bool,
    ) -> Result<SwapQuoteFFI, OubliError> {
        let mut core = self.core.lock().unwrap();
        let quote = self
            .runtime
            .block_on(core.handle_swap_wbtc_to_btc(amount_sats, &btc_address, exact_in))
            .map_err(OubliError::from)?;
        Ok(swap_quote_to_ffi(quote))
    }

    pub fn swap_ln_to_wbtc(
        &self,
        amount_sats: u64,
        exact_in: bool,
    ) -> Result<SwapQuoteFFI, OubliError> {
        let mut core = self.core.lock().unwrap();
        let quote = self
            .runtime
            .block_on(core.handle_swap_ln_to_wbtc(amount_sats, exact_in))
            .map_err(OubliError::from)?;
        Ok(swap_quote_to_ffi(quote))
    }

    pub fn swap_execute(&self, swap_id: String) -> Result<(), OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_swap_execute(&swap_id))
            .map_err(OubliError::from)
    }

    pub fn swap_status(&self, swap_id: String) -> Result<SwapStatusFFI, OubliError> {
        let mut core = self.core.lock().unwrap();
        let status = self
            .runtime
            .block_on(core.handle_swap_status(&swap_id))
            .map_err(OubliError::from)?;
        Ok(SwapStatusFFI {
            swap_id: status.swap_id,
            state: format!("{:?}", status.state),
            tx_id: status.tx_id,
        })
    }

    pub fn swap_list(&self) -> Result<Vec<SwapSummaryFFI>, OubliError> {
        let mut core = self.core.lock().unwrap();
        let swaps = self
            .runtime
            .block_on(core.handle_swap_list())
            .map_err(OubliError::from)?;
        Ok(swaps
            .into_iter()
            .map(|s| SwapSummaryFFI {
                swap_id: s.swap_id,
                state: format!("{:?}", s.state),
                input_amount: s.input_amount,
                output_amount: s.output_amount,
            })
            .collect())
    }

    pub fn swap_limits(&self, direction: String) -> Result<SwapLimitsFFI, OubliError> {
        let mut core = self.core.lock().unwrap();
        let limits = self
            .runtime
            .block_on(core.handle_swap_limits(&direction))
            .map_err(OubliError::from)?;
        Ok(SwapLimitsFFI {
            input_min: limits.input.min,
            input_max: limits.input.max,
            output_min: limits.output.min,
            output_max: limits.output.max,
        })
    }

    /// Wait for an incoming Lightning payment and claim WBTC.
    /// Call after `swap_ln_to_wbtc` to block until the payer pays the invoice.
    pub fn receive_lightning_wait(&self, swap_id: String) -> Result<(), OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_receive_lightning_wait(&swap_id))
            .map_err(OubliError::from)
    }

    /// Pay a Lightning invoice by scanning a BOLT11 QR code.
    /// Orchestrates: withdraw from Tongo → Atomiq WBTC→BTCLN swap → LP pays invoice.
    /// Returns the swap ID on success.
    pub fn pay_lightning(&self, bolt11: String) -> Result<String, OubliError> {
        let mut core = self.core.lock().unwrap();
        self.runtime
            .block_on(core.handle_pay_lightning(&bolt11))
            .map_err(OubliError::from)
    }

    /// Calculate the fee in sats for a given amount.
    /// Returns "0" if no fee is configured.
    pub fn calculate_fee(&self, amount_sats: String) -> String {
        let core = self.core.lock().unwrap();
        let config = core.config();
        if config.fee_collector_pubkey.is_none() || config.fee_percent <= 0.0 {
            return "0".into();
        }
        let sats: u64 = amount_sats.trim().parse().unwrap_or(0);
        let fee = oubli_wallet::calculate_fee_sats(sats, config.fee_percent);
        fee.to_string()
    }

    /// Calculate the effective fee for `handle_send`, accounting for recipient routing.
    pub fn calculate_send_fee(&self, amount_sats: String, recipient: String) -> String {
        let core = self.core.lock().unwrap();
        calculate_send_fee_for_recipient(&core, &amount_sats, &recipient)
    }

    /// Returns the configured fee percentage (e.g. 1.0 for 1%).
    pub fn get_fee_percent(&self) -> f64 {
        let core = self.core.lock().unwrap();
        core.config().fee_percent
    }

    // ── Contacts ─────────────────────────────────────────────

    pub fn get_contacts(&self) -> Vec<ContactFFI> {
        let core = self.core.lock().unwrap();
        oubli_wallet::contacts::get_contacts(core.storage())
            .into_iter()
            .map(ContactFFI::from)
            .collect()
    }

    pub fn find_contact_by_address(&self, address: String) -> Option<ContactFFI> {
        let core = self.core.lock().unwrap();
        oubli_wallet::contacts::find_contact_by_address(core.storage(), &address)
            .map(ContactFFI::from)
    }

    pub fn save_contact(&self, contact: ContactFFI) -> Result<String, OubliError> {
        let core = self.core.lock().unwrap();
        oubli_wallet::contacts::save_contact(core.storage(), contact.into())
            .map_err(OubliError::from)
    }

    pub fn delete_contact(&self, contact_id: String) -> Result<(), OubliError> {
        let core = self.core.lock().unwrap();
        oubli_wallet::contacts::delete_contact(core.storage(), &contact_id)
            .map_err(OubliError::from)
    }

    pub fn update_contact_last_used(&self, contact_id: String) -> Result<(), OubliError> {
        let core = self.core.lock().unwrap();
        oubli_wallet::contacts::update_contact_last_used(core.storage(), &contact_id)
            .map_err(OubliError::from)
    }

    pub fn get_transfer_recipient(&self, tx_hash: String) -> Option<String> {
        let core = self.core.lock().unwrap();
        core.get_transfer_recipient(&tx_hash)
    }
}

fn swap_quote_to_ffi(q: oubli_swap::types::SwapQuote) -> SwapQuoteFFI {
    SwapQuoteFFI {
        swap_id: q.swap_id,
        input_amount: q.input_amount,
        output_amount: q.output_amount,
        fee: q.fee,
        expiry: q.expiry,
        btc_address: q.btc_address,
        ln_invoice: q.ln_invoice,
    }
}

fn calculate_send_fee_for_recipient(
    core: &WalletCore,
    amount_sats: &str,
    recipient: &str,
) -> String {
    let config = core.config();
    if config.fee_collector_pubkey.is_none() || config.fee_percent <= 0.0 {
        return "0".into();
    }

    let trimmed = recipient.trim();
    if trimmed.is_empty() {
        return "0".into();
    }
    let stripped = trimmed.strip_prefix("0x").unwrap_or(trimmed);

    // `handle_send` routes long hex strings to private transfers. Lightning
    // invoices also exceed the Starknet address length, so they remain free.
    if stripped.len() > 64 {
        return "0".into();
    }

    if let Some(address) = current_starknet_address(core) {
        if normalize_hex_id(address) == normalize_hex_id(trimmed) {
            return "0".into();
        }
    }

    let sats: u64 = amount_sats.trim().parse().unwrap_or(0);
    oubli_wallet::calculate_fee_sats(sats, config.fee_percent).to_string()
}

fn current_starknet_address(core: &WalletCore) -> Option<&str> {
    match core.state() {
        WalletState::Ready { address, .. } | WalletState::Processing { address, .. } => {
            Some(address.as_str())
        }
        _ => None,
    }
}

fn normalize_hex_id(value: &str) -> String {
    let trimmed = value.trim();
    let stripped = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let without_leading_zeros = stripped.trim_start_matches('0');
    if without_leading_zeros.is_empty() {
        "0".into()
    } else {
        without_leading_zeros.to_ascii_lowercase()
    }
}

// ── BTC price helper ─────────────────────────────────────────

/// Fetch BTC price in the given fiat currency from CoinGecko on a dedicated thread.
fn fetch_btc_price_blocking(currency: &str) -> Option<f64> {
    let currency = currency.to_lowercase();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Oubli-Wallet/0.1")
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                crate::bridge_warn_event!(
                    "bridge.btc_price",
                    "client_build_failed",
                    "error_kind" = telemetry::error_kind(&e)
                );
                let _ = tx.send(None);
                return;
            }
        };
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies={}",
            currency
        );
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(e) => {
                crate::bridge_warn_event!(
                    "bridge.btc_price",
                    "request_failed",
                    "error_kind" = telemetry::error_kind(&e)
                );
                let _ = tx.send(None);
                return;
            }
        };
        let json: serde_json::Value = match resp.json() {
            Ok(j) => j,
            Err(e) => {
                crate::bridge_warn_event!(
                    "bridge.btc_price",
                    "json_parse_failed",
                    "error_kind" = telemetry::error_kind(&e)
                );
                let _ = tx.send(None);
                return;
            }
        };
        let price = json
            .get("bitcoin")
            .and_then(|b| b.get(&currency))
            .and_then(|u| u.as_f64());
        let _ = tx.send(price);
    });
    rx.recv_timeout(Duration::from_secs(15)).ok().flatten()
}

uniffi::include_scaffolding!("oubli");

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a CoinGecko-style JSON response and extract the BTC/USD price.
    fn parse_btc_price(json: &serde_json::Value) -> Option<f64> {
        json.get("bitcoin")?.get("usd")?.as_f64()
    }

    #[test]
    fn test_parse_btc_price_valid_response() {
        let json: serde_json::Value = serde_json::json!({"bitcoin": {"usd": 70499.0}});
        let price = parse_btc_price(&json);
        assert_eq!(price, Some(70499.0));
    }

    #[test]
    fn test_parse_btc_price_missing_bitcoin_key() {
        let json: serde_json::Value = serde_json::json!({"ethereum": {"usd": 3500.0}});
        assert_eq!(parse_btc_price(&json), None);
    }

    #[test]
    fn test_parse_btc_price_missing_usd_key() {
        let json: serde_json::Value = serde_json::json!({"bitcoin": {"eur": 65000.0}});
        assert_eq!(parse_btc_price(&json), None);
    }

    #[test]
    fn test_parse_btc_price_error_response() {
        let json: serde_json::Value = serde_json::json!({
            "status": {"error_code": 429, "error_message": "Rate limited"}
        });
        assert_eq!(parse_btc_price(&json), None);
    }

    #[test]
    fn test_btc_price_cache_hit() {
        let cache: Mutex<Option<(f64, std::time::Instant)>> = Mutex::new(None);

        // Empty cache
        assert!(cache.lock().unwrap().is_none());

        // Store a price
        let price = 70000.0;
        *cache.lock().unwrap() = Some((price, std::time::Instant::now()));

        // Cache hit within 1 hour
        let cached = cache.lock().unwrap();
        let (cached_price, fetched_at) = cached.as_ref().unwrap();
        assert_eq!(*cached_price, price);
        assert!(fetched_at.elapsed() < Duration::from_secs(3600));
    }

    #[test]
    fn test_btc_price_cache_expired() {
        let cache: Mutex<Option<(f64, std::time::Instant)>> = Mutex::new(None);

        // Store a price with a timestamp 2 hours in the past
        let old_time = std::time::Instant::now() - Duration::from_secs(7200);
        *cache.lock().unwrap() = Some((60000.0, old_time));

        // Cache should be considered expired
        let cached = cache.lock().unwrap();
        let (_, fetched_at) = cached.as_ref().unwrap();
        assert!(fetched_at.elapsed() >= Duration::from_secs(3600));
    }

    #[test]
    #[ignore] // Hits external API — run with --ignored
    fn test_fetch_btc_price_live() {
        let price = fetch_btc_price_blocking("usd");
        assert!(price.is_some(), "CoinGecko should return a BTC price");
        let price = price.unwrap();
        assert!(price > 1_000.0, "BTC price should be > $1,000, got {price}");
        assert!(
            price < 10_000_000.0,
            "BTC price should be < $10M, got {price}"
        );
        assert!(price.is_finite(), "Price should be finite");
    }
}

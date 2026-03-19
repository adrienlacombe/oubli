use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
            oubli_wallet::WalletError::InsufficientBalance { .. } => OubliError::InsufficientBalance,
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
}

impl OubliWallet {
    pub fn new(storage: Box<dyn PlatformStorageCallback>) -> Result<Self, OubliError> {
        let adapter = PlatformStorageAdapter { callback: storage };
        let config = NetworkConfig::from_env();
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

        // Spawn background polling thread.
        // Uses try_lock so it never blocks user-initiated operations.
        {
            let poll_core = Arc::clone(&core);
            let poll_handle = runtime.handle().clone();
            let poll_flag = Arc::clone(&polling_active);
            std::thread::Builder::new()
                .name("oubli-poll".into())
                .spawn(move || {
                    loop {
                        std::thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
                        if !poll_flag.load(Ordering::Relaxed) {
                            continue;
                        }
                        if let Ok(mut core) = poll_core.try_lock() {
                            let _ = poll_handle.block_on(core.handle_refresh_balance());
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
        })
    }

    pub fn get_state(&self) -> WalletStateInfo {
        let core = self.core.lock().unwrap();
        wallet_state_to_ffi(core.state(), core.owner_public_key(), core.last_auto_fund_error())
    }

    pub fn handle_complete_onboarding(
        &self,
        mnemonic: String,
    ) -> Result<(), OubliError> {
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
        let _ = core.update_rpc_url(url);
    }

    pub fn get_rpc_url(&self) -> String {
        let core = self.core.lock().unwrap();
        core.rpc_url().to_string()
    }

    pub fn handle_start_seed_backup(
        &self,
        mnemonic: String,
    ) -> Result<SeedBackupStateFFI, OubliError> {
        let flow =
            SeedDisplayFlow::new(&mnemonic).map_err(|e| backup_err(e.to_string()))?;

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
        let mut core = self.core.lock().unwrap();
        self.runtime.block_on(core.get_btc_price_usd())
    }

    pub fn get_activity(&self) -> Result<Vec<ActivityEventFFI>, OubliError> {
        let core = self.core.lock().unwrap();
        let events = self.runtime.block_on(core.get_activity())?;
        Ok(events
            .into_iter()
            .map(|e| ActivityEventFFI {
                event_type: e.event_type,
                amount_sats: e.amount_sats,
                tx_hash: e.tx_hash,
                block_number: e.block_number,
            })
            .collect())
    }

    pub fn get_cached_activity(&self) -> Vec<ActivityEventFFI> {
        let core = self.core.lock().unwrap();
        core.get_cached_activity()
            .into_iter()
            .map(|e| ActivityEventFFI {
                event_type: e.event_type,
                amount_sats: e.amount_sats,
                tx_hash: e.tx_hash,
                block_number: e.block_number,
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
        let core = self.core.lock().unwrap();
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
        let core = self.core.lock().unwrap();
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

uniffi::include_scaffolding!("oubli");

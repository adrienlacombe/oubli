/// User-initiated actions that drive the wallet state machine.
#[derive(Debug, Clone)]
pub enum UserAction {
    /// Complete onboarding with a new or restored mnemonic.
    CompleteOnboarding { mnemonic: String, pin: String },
    /// Unlock via biometric.
    UnlockBiometric,
    /// Unlock via PIN.
    UnlockPin { pin: String },
    /// Fund the account (deposit).
    Fund { amount_sats: String },
    /// Transfer to another Tongo user.
    Transfer { amount_sats: String, recipient_pub_key: String },
    /// Withdraw to an L1 address.
    Withdraw { amount_sats: String, recipient_address: String },
    /// Rollover pending balance.
    Rollover,
    /// Rage-quit: withdraw everything.
    RageQuit { recipient_address: String },
    /// Start seed backup flow.
    StartSeedBackup,
    /// Lock the wallet.
    Lock,
    /// App went to background.
    Background,
    /// Dismiss an error.
    DismissError,
}

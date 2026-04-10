/// Top-level wallet state — no secrets, FFI-safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletState {
    /// First launch — needs setup.
    Onboarding,
    /// Device locked.
    Locked,
    /// Biometric passed — can transact.
    Ready {
        address: String,
        balance_sats: String,
        pending_sats: String,
    },
    /// Operation in flight.
    Processing { address: String, operation: String },
    /// An error the user needs to dismiss.
    Error { message: String },
    /// Seed backup flow in progress.
    SeedBackup,
    /// Wallet has been wiped.
    Wiped,
}

impl WalletState {
    /// Whether the wallet has an active account loaded.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            WalletState::Ready { .. } | WalletState::Processing { .. } | WalletState::SeedBackup
        )
    }

    /// Short description for logging / display.
    pub fn label(&self) -> &'static str {
        match self {
            WalletState::Onboarding => "onboarding",
            WalletState::Locked => "locked",
            WalletState::Ready { .. } => "ready",
            WalletState::Processing { .. } => "processing",
            WalletState::Error { .. } => "error",
            WalletState::SeedBackup => "seed_backup",
            WalletState::Wiped => "wiped",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_states() {
        assert!(!WalletState::Onboarding.is_active());
        assert!(!WalletState::Locked.is_active());
        assert!(!WalletState::Wiped.is_active());
        assert!(WalletState::Ready {
            address: String::new(),
            balance_sats: String::new(),
            pending_sats: String::new(),
        }
        .is_active());
        assert!(WalletState::SeedBackup.is_active());
    }

    #[test]
    fn labels() {
        assert_eq!(WalletState::Onboarding.label(), "onboarding");
        assert_eq!(WalletState::Locked.label(), "locked");
        assert_eq!(WalletState::Wiped.label(), "wiped");
    }
}

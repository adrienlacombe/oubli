use crate::error::BackupError;

/// Restore flow — validates a mnemonic phrase using the `krusty_kms` crate.
pub struct RestoreFlow;

impl RestoreFlow {
    /// Validate that the given mnemonic is a well-formed BIP-39 phrase.
    /// Delegates to `krusty_kms::validate_mnemonic`.
    pub fn validate_mnemonic(phrase: &str) -> Result<(), BackupError> {
        krusty_kms::validate_mnemonic(phrase)
            .map_err(|e| BackupError::InvalidMnemonic(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_mnemonic_passes() {
        // Generate a fresh mnemonic via krusty_kms and validate it
        let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
        assert!(RestoreFlow::validate_mnemonic(&mnemonic).is_ok());
    }

    #[test]
    fn invalid_mnemonic_fails() {
        assert!(RestoreFlow::validate_mnemonic("not a valid mnemonic phrase").is_err());
    }

    #[test]
    fn empty_mnemonic_fails() {
        assert!(RestoreFlow::validate_mnemonic("").is_err());
    }
}

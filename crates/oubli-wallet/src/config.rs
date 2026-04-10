use crate::error::WalletError;

/// Network configuration for connecting to Starknet.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// JSON-RPC endpoint URL.
    pub rpc_url: String,
    /// Starknet chain identifier (e.g. SN_SEPOLIA, SN_MAIN).
    pub chain_id: String,
    /// Tongo contract address (hex).
    pub tongo_contract: String,
    /// ERC-20 token contract address (hex).
    pub token_contract: String,
    /// Counterfactual Starknet account class hash (hex).
    pub account_class_hash: String,
    /// Paymaster URL for AVNU gasless transactions.
    pub paymaster_url: String,
    /// Optional AVNU paymaster API key for gasfree (sponsored) mode.
    pub paymaster_api_key: Option<String>,
    /// Fee percentage charged on external withdraws (e.g. 1.0 = 1%).
    pub fee_percent: f64,
    /// Tongo public key (128 hex chars) of the fee collector wallet.
    pub fee_collector_pubkey: Option<String>,
}

impl NetworkConfig {
    pub fn normalize_rpc_url(url: &str) -> Result<String, WalletError> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err(WalletError::Rpc("invalid RPC URL: empty URL".into()));
        }

        let parsed = reqwest::Url::parse(trimmed)
            .map_err(|e| WalletError::Rpc(format!("invalid RPC URL: {e}")))?;
        let scheme = parsed.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(WalletError::Rpc(format!(
                "invalid RPC URL: unsupported scheme '{scheme}'"
            )));
        }
        if parsed.host_str().is_none() {
            return Err(WalletError::Rpc("invalid RPC URL: missing host".into()));
        }

        Ok(trimmed.to_string())
    }

    pub fn set_rpc_url(&mut self, url: &str) -> Result<(), WalletError> {
        self.rpc_url = Self::normalize_rpc_url(url)?;
        Ok(())
    }

    fn defaults_for(selected_network: Option<&str>, selected_chain_id: Option<&str>) -> Self {
        match selected_network.or(selected_chain_id) {
            Some("sepolia") | Some("SN_SEPOLIA") => crate::networks::sepolia::config(),
            _ => crate::networks::mainnet::config(),
        }
    }

    /// Build a config from environment variables, defaulting to mainnet when
    /// no explicit network has been requested.
    pub fn from_env() -> Self {
        let selected_network = std::env::var("OUBLI_NETWORK").ok();
        let selected_chain_id = std::env::var("OUBLI_CHAIN_ID").ok();
        let defaults =
            Self::defaults_for(selected_network.as_deref(), selected_chain_id.as_deref());
        Self {
            rpc_url: std::env::var("OUBLI_RPC_URL").unwrap_or(defaults.rpc_url),
            chain_id: std::env::var("OUBLI_CHAIN_ID").unwrap_or(defaults.chain_id),
            tongo_contract: std::env::var("OUBLI_TONGO_CONTRACT")
                .unwrap_or(defaults.tongo_contract),
            token_contract: std::env::var("OUBLI_TOKEN_CONTRACT")
                .unwrap_or(defaults.token_contract),
            account_class_hash: std::env::var("OUBLI_ACCOUNT_CLASS_HASH")
                .unwrap_or(defaults.account_class_hash),
            paymaster_url: std::env::var("OUBLI_PAYMASTER_URL").unwrap_or(defaults.paymaster_url),
            paymaster_api_key: std::env::var("OUBLI_AVNU_PAYMASTER_API_KEY")
                .ok()
                .or(defaults.paymaster_api_key),
            fee_percent: defaults.fee_percent,
            fee_collector_pubkey: defaults.fee_collector_pubkey,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_produces_valid_config() {
        let cfg = NetworkConfig::from_env();
        assert!(
            cfg.chain_id == "SN_MAIN" || cfg.chain_id == "SN_SEPOLIA",
            "chain_id should be SN_MAIN or SN_SEPOLIA, got {}",
            cfg.chain_id
        );
        assert!(!cfg.tongo_contract.is_empty());
    }

    #[test]
    fn defaults_to_mainnet_when_no_network_is_selected() {
        let cfg = NetworkConfig::defaults_for(None, None);
        assert_eq!(cfg.chain_id, "SN_MAIN");
        assert!(!cfg.tongo_contract.is_empty());
    }

    #[test]
    fn selects_mainnet_when_requested() {
        let cfg = NetworkConfig::defaults_for(Some("mainnet"), None);
        assert_eq!(cfg.chain_id, "SN_MAIN");
    }

    #[test]
    fn normalize_rpc_url_accepts_absolute_http_urls() {
        let normalized =
            NetworkConfig::normalize_rpc_url("  https://rpc.example.com/v1  ").unwrap();
        assert_eq!(normalized, "https://rpc.example.com/v1");
    }

    #[test]
    fn normalize_rpc_url_rejects_relative_urls() {
        let err = NetworkConfig::normalize_rpc_url("/rpc").unwrap_err();
        assert!(err.to_string().contains("invalid RPC URL"));
    }

    #[test]
    fn set_rpc_url_rejects_invalid_values_without_mutating_config() {
        let mut cfg = NetworkConfig {
            rpc_url: "https://rpc.example.com".into(),
            chain_id: "SN_SEPOLIA".into(),
            tongo_contract: "0x1".into(),
            token_contract: "0x2".into(),
            account_class_hash: "0x3".into(),
            paymaster_url: "https://paymaster.example.com".into(),
            paymaster_api_key: None,
            fee_percent: 0.0,
            fee_collector_pubkey: None,
        };

        let err = cfg.set_rpc_url("/rpc").unwrap_err();
        assert!(err.to_string().contains("invalid RPC URL"));
        assert_eq!(cfg.rpc_url, "https://rpc.example.com");
    }
}

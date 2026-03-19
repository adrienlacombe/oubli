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
    /// OZ account class hash (hex).
    pub account_class_hash: String,
    /// Paymaster URL for AVNU gasless transactions.
    pub paymaster_url: String,
    /// Optional AVNU paymaster API key for gasfree (sponsored) mode.
    pub paymaster_api_key: Option<String>,
}

impl NetworkConfig {
    /// Build a config from environment variables, falling back to mainnet defaults.
    pub fn from_env() -> Self {
        let defaults = crate::networks::mainnet::config();
        Self {
            rpc_url: std::env::var("OUBLI_RPC_URL").unwrap_or(defaults.rpc_url),
            chain_id: std::env::var("OUBLI_CHAIN_ID").unwrap_or(defaults.chain_id),
            tongo_contract: std::env::var("OUBLI_TONGO_CONTRACT")
                .unwrap_or(defaults.tongo_contract),
            token_contract: std::env::var("OUBLI_TOKEN_CONTRACT")
                .unwrap_or(defaults.token_contract),
            account_class_hash: std::env::var("OUBLI_ACCOUNT_CLASS_HASH")
                .unwrap_or(defaults.account_class_hash),
            paymaster_url: std::env::var("OUBLI_PAYMASTER_URL")
                .unwrap_or(defaults.paymaster_url),
            paymaster_api_key: std::env::var("OUBLI_AVNU_PAYMASTER_API_KEY")
                .ok()
                .or(defaults.paymaster_api_key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env_defaults_to_mainnet() {
        // env!() values are baked at compile time; from_env() can override at runtime
        let cfg = NetworkConfig::from_env();
        assert_eq!(cfg.chain_id, "SN_MAIN");
        assert!(!cfg.rpc_url.is_empty());
        assert!(!cfg.tongo_contract.is_empty());
    }
}

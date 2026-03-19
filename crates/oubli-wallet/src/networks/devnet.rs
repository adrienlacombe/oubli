use crate::config::NetworkConfig;

/// Build a devnet config. The `tongo_contract` is only known after deployment,
/// so it must be passed as a parameter.
pub fn config(tongo_contract: &str) -> NetworkConfig {
    NetworkConfig {
        rpc_url: "http://localhost:5050".into(),
        chain_id: "SN_SEPOLIA".into(), // starknet-devnet default
        tongo_contract: tongo_contract.into(),
        token_contract: "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d"
            .into(), // devnet STRK
        account_class_hash:
            "0x05b4b537eaa2399e3aa99c4e2e0208ebd6c71bc1467938cd52c798c601e43564".into(),
        paymaster_url: String::new(),
        paymaster_api_key: None,
    }
}

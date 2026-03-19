use crate::config::NetworkConfig;

pub fn config() -> NetworkConfig {
    NetworkConfig {
        rpc_url: env!("OUBLI_SEPOLIA_RPC_URL").into(),
        chain_id: "SN_SEPOLIA".into(),
        tongo_contract: "0x0408163bfcfc2d76f34b444cb55e09dace5905cf84c0884e4637c2c0f06ab6ed".into(),
        token_contract: "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d".into(),
        account_class_hash: "0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f".into(),
        paymaster_url: "https://sepolia.paymaster.avnu.fi".into(),
        paymaster_api_key: option_env!("OUBLI_SEPOLIA_PAYMASTER_API_KEY").map(Into::into),
    }
}

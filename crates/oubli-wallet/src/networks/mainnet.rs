use crate::config::NetworkConfig;

pub fn config() -> NetworkConfig {
    NetworkConfig {
        rpc_url: env!("OUBLI_MAINNET_RPC_URL").into(),
        chain_id: "SN_MAIN".into(),
        tongo_contract: "0x012ddbba903d9a22d3169b59c3de21affc1557d2d61b91646dfccd69b79b7120".into(),
        token_contract: "0x03fe2b97c1fd336e750087d68b9b867997fd64a2661ff3ca5a7c771641e8e7ac".into(),
        account_class_hash: "0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f".into(),
        paymaster_url: "https://starknet.paymaster.avnu.fi".into(),
        paymaster_api_key: option_env!("OUBLI_MAINNET_PAYMASTER_API_KEY").map(Into::into),
    }
}

use crate::config::NetworkConfig;

pub fn config() -> NetworkConfig {
    let default_rpc = super::secrets::mainnet_rpc();
    let default_key = super::secrets::mainnet_paymaster();
    let default_fee_collector = super::secrets::mainnet_fee_collector();
    let default_fee_percent = super::secrets::mainnet_fee_percent();
    NetworkConfig {
        rpc_url: std::env::var("OUBLI_MAINNET_RPC_URL").unwrap_or(default_rpc),
        chain_id: "SN_MAIN".into(),
        tongo_contract: "0x012ddbba903d9a22d3169b59c3de21affc1557d2d61b91646dfccd69b79b7120".into(),
        token_contract: "0x03fe2b97c1fd336e750087d68b9b867997fd64a2661ff3ca5a7c771641e8e7ac".into(),
        account_class_hash: "0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f"
            .into(),
        paymaster_url: "https://starknet.paymaster.avnu.fi".into(),
        paymaster_api_key: std::env::var("OUBLI_MAINNET_PAYMASTER_API_KEY")
            .ok()
            .or_else(|| {
                if default_key.is_empty() {
                    None
                } else {
                    Some(default_key)
                }
            }),
        fee_percent: std::env::var("OUBLI_FEE_PERCENT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| default_fee_percent.parse().unwrap_or(0.0)),
        fee_collector_pubkey: std::env::var("OUBLI_FEE_COLLECTOR_PUBKEY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if default_fee_collector.is_empty() {
                    None
                } else {
                    Some(default_fee_collector)
                }
            }),
    }
}

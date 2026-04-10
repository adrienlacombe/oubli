#[cfg(feature = "devnet")]
pub mod devnet;
pub mod mainnet;
pub mod sepolia;

pub(crate) mod secrets {
    include!(concat!(env!("OUT_DIR"), "/secrets.rs"));

    pub fn mainnet_rpc() -> String {
        decode(MAINNET_RPC_ENC)
    }
    pub fn mainnet_paymaster() -> String {
        decode(MAINNET_PAYMASTER_ENC)
    }
    pub fn mainnet_fee_collector() -> String {
        decode(MAINNET_FEE_COLLECTOR_ENC)
    }
    pub fn mainnet_fee_percent() -> String {
        decode(MAINNET_FEE_PERCENT_ENC)
    }
    pub fn sepolia_rpc() -> String {
        decode(SEPOLIA_RPC_ENC)
    }
    pub fn sepolia_paymaster() -> String {
        decode(SEPOLIA_PAYMASTER_ENC)
    }
}

#![deny(unsafe_code)]

pub mod actions;
pub mod config;
pub mod core;
pub mod networks;
pub mod denomination;
pub mod error;
pub mod paymaster;
pub mod queue;
pub mod rpc;
pub mod operations;
pub mod signing;
pub mod state;
pub mod submitter;
pub mod swap;

pub use actions::UserAction;
pub use config::NetworkConfig;
pub use self::core::{ActiveAccount, ActivityEvent, WalletCore};
pub use denomination::{sats_to_tongo_units, format_sats_display, tongo_units_to_sats};
// Keep old names as re-exports during migration
pub use denomination::{btc_to_tongo_units, format_btc_display, tongo_units_to_btc};
pub use error::WalletError;
pub use paymaster::PaymasterClient;
pub use queue::OperationQueue;
pub use rpc::RpcClient;
pub use state::WalletState;
pub use submitter::TransactionSubmitter;

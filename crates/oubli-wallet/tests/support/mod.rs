use oubli_wallet::config::NetworkConfig;
use oubli_wallet::core::ActiveAccount;
use oubli_wallet::rpc::RpcClient;
use oubli_wallet::submitter::{PaymasterSubmitter, TransactionSubmitter};
use starknet_types_core::felt::Felt;

pub fn faucet_starknet_address(config: &NetworkConfig) -> Felt {
    use krusty_kms_client::starknet_rust::core::types::Felt as RsFelt;
    use krusty_kms_client::starknet_rust::signers::SigningKey;

    let mnemonic_a = std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A");
    let sk = krusty_kms::derive_private_key_with_coin_type(&mnemonic_a, 0, 0, 9004, None)
        .expect("faucet key derivation failed");
    let sk_rs = RsFelt::from_bytes_be(&sk.to_bytes_be());
    let signing_key = SigningKey::from_secret_scalar(sk_rs);
    let pub_key_rs = signing_key.verifying_key().scalar();

    let class_hash_rs =
        RsFelt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let address_rs = krusty_kms_client::starknet_rust::core::utils::get_contract_address(
        RsFelt::ZERO,
        class_hash_rs,
        &[RsFelt::ZERO, pub_key_rs, RsFelt::ONE],
        RsFelt::ZERO,
    );

    Felt::from_bytes_be(&address_rs.to_bytes_be())
}

fn faucet_active_account(config: &NetworkConfig) -> ActiveAccount {
    use krusty_kms_client::starknet_rust::core::types::Felt as RsFelt;
    use krusty_kms_client::starknet_rust::signers::SigningKey;

    let mnemonic_a = std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A");
    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).expect("invalid tongo contract address");
    let tongo_account =
        krusty_kms_sdk::TongoAccount::from_mnemonic(&mnemonic_a, 0, 0, tongo_addr, None)
            .expect("faucet tongo account derivation failed");

    let starknet_sk = krusty_kms::derive_private_key_with_coin_type(&mnemonic_a, 0, 0, 9004, None)
        .expect("faucet starknet key derivation failed");
    let sk_rs = RsFelt::from_bytes_be(&starknet_sk.to_bytes_be());
    let signing_key = SigningKey::from_secret_scalar(sk_rs);
    let starknet_pub_key_rs = signing_key.verifying_key().scalar();
    let starknet_public_key = Felt::from_bytes_be(&starknet_pub_key_rs.to_bytes_be());

    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let class_hash_rs = RsFelt::from_bytes_be(&class_hash.to_bytes_be());
    let starknet_address_rs = krusty_kms_client::starknet_rust::core::utils::get_contract_address(
        RsFelt::ZERO,
        class_hash_rs,
        &[RsFelt::ZERO, starknet_pub_key_rs, RsFelt::ONE],
        RsFelt::ZERO,
    );
    let starknet_address = Felt::from_bytes_be(&starknet_address_rs.to_bytes_be());
    let owner_public_key_hex = tongo_account
        .owner_public_key_hex()
        .expect("faucet owner public key");

    ActiveAccount {
        tongo_account,
        starknet_private_key: starknet_sk,
        starknet_address,
        starknet_public_key: starknet_public_key,
        owner_public_key_hex,
        balance: 0,
        pending: 0,
        nonce: Felt::ZERO,
        cipher_balance: None,
        auditor_key: None,
    }
}

#[allow(dead_code)]
pub async fn ensure_faucet_deployed_via_paymaster(config: &NetworkConfig) -> Option<String> {
    let faucet = faucet_active_account(config);
    let rpc = RpcClient::new(config).expect("RpcClient");
    let submitter =
        PaymasterSubmitter::new(&config.paymaster_url, config.paymaster_api_key.as_deref());

    submitter
        .deploy_account(&faucet, config, &rpc)
        .await
        .expect("faucet deploy via paymaster failed")
}

pub async fn faucet_transfer_via_paymaster(
    config: &NetworkConfig,
    to_address: &Felt,
    amount: u128,
) -> String {
    use krusty_kms_client::starknet_rust::core::types::{Call as RsCall, Felt as RsFelt};
    use krusty_kms_client::starknet_rust::core::utils::get_selector_from_name;

    let faucet = faucet_active_account(config);
    let rpc = RpcClient::new(config).expect("RpcClient");
    let submitter =
        PaymasterSubmitter::new(&config.paymaster_url, config.paymaster_api_key.as_deref());

    let erc20 = RsFelt::from_hex(&config.token_contract).expect("invalid token contract");
    let recipient = RsFelt::from_bytes_be(&to_address.to_bytes_be());
    let transfer_call = RsCall {
        to: erc20,
        selector: get_selector_from_name("transfer").expect("selector"),
        calldata: vec![recipient, RsFelt::from(amount), RsFelt::ZERO],
    };

    submitter
        .ensure_deployed(&faucet, config, &rpc)
        .await
        .expect("faucet ensure_deployed failed");
    submitter
        .submit(&faucet, vec![transfer_call])
        .await
        .expect("faucet transfer via paymaster failed")
}

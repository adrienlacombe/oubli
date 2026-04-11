mod support;

use oubli_store::MockPlatformStorage;
use oubli_wallet::config::NetworkConfig;
use oubli_wallet::core::WalletCore;
use oubli_wallet::networks;
use oubli_wallet::rpc::RpcClient;
use oubli_wallet::state::WalletState;
use serial_test::serial;
use starknet_types_core::felt::Felt;
use support::{faucet_starknet_address, faucet_transfer_via_paymaster};

fn test_config() -> NetworkConfig {
    NetworkConfig::from_env()
}

/// Send STRK from the faucet wallet (OUBLI_TEST_MNEMONIC_A) to a target address.
///
/// Uses the AVNU paymaster directly to avoid WalletCore's auto-fund, which would
/// sweep the faucet's public tokens into the Tongo privacy pool.
async fn faucet_strk(config: &NetworkConfig, to_address: &Felt, amount: u128) {
    let tx_hash = faucet_transfer_via_paymaster(config, to_address, amount).await;
    eprintln!("faucet_strk tx: {tx_hash}");

    // Wait for confirmation
    let rpc = RpcClient::new(config).expect("RpcClient");
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(&tx_hash).await {
            eprintln!("faucet_strk confirmed after {attempt} attempts");
            return;
        }
        eprintln!("faucet_strk attempt {attempt}: waiting for confirmation...");
    }
    panic!("faucet_strk tx {tx_hash} was not confirmed in time");
}

#[tokio::test]
async fn test_full_lifecycle_mock() {
    // 1. Create wallet — starts in Onboarding
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, test_config());
    assert!(matches!(wallet.state(), WalletState::Onboarding));

    // 2. Generate a valid mnemonic for testing
    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();

    // 3. Complete onboarding — uses mock storage, RPC will fail but
    //    onboarding still succeeds (fresh account has no on-chain state)
    let result = wallet.handle_onboarding(&mnemonic).await;
    assert!(result.is_ok(), "onboarding failed: {:?}", result.err());
    assert!(matches!(wallet.state(), WalletState::Ready { .. }));

    // 4. Verify active account is populated
    assert!(wallet.active_account().is_some());
    let acct = wallet.active_account().unwrap();
    assert!(!acct.owner_public_key_hex.is_empty());

    // 5. Lock — clears active account
    wallet.handle_lock();
    assert!(matches!(wallet.state(), WalletState::Locked));
    assert!(wallet.active_account().is_none());

    // 6. Biometric unlock → Ready (decrypts seed, derives keys)
    let result = wallet.handle_unlock_biometric().await;
    assert!(result.is_ok());
    assert!(matches!(wallet.state(), WalletState::Ready { .. }));
    assert!(wallet.active_account().is_some());

    // 7. Background → drops to Locked, clears active account
    wallet.handle_background();
    assert!(wallet.active_account().is_none());
    assert!(matches!(wallet.state(), WalletState::Locked));

    // 8. Lock fully
    wallet.handle_lock();
    assert!(matches!(wallet.state(), WalletState::Locked));
}

#[tokio::test]
async fn test_fund_requires_t2() {
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, test_config());

    // At Onboarding/T0 — fund should fail
    let result = wallet.handle_fund("0.001").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("T2Transact"));
}

#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_fund_and_rollover_sepolia() {
    let config = NetworkConfig::from_env();

    // Fresh mnemonic — guaranteed undeployed, zero balance.
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");

    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());
    wallet
        .handle_onboarding(&mnemonic)
        .await
        .expect("onboarding failed");

    let starknet_addr = wallet.active_account().unwrap().starknet_address;

    // Faucet sends STRK so the ERC-20 approve inside fund() can succeed.
    faucet_strk(&config, &starknet_addr, 1_000_000_000_000_000_000).await;

    // Fund: deploy + deposit via paymaster
    let tx = wallet.handle_fund("10").await.expect("fund failed");
    assert!(!tx.is_empty());
    eprintln!("fund tx hash: {tx}");

    // Wait for confirmation
    let rpc = RpcClient::new(&config).expect("RpcClient");
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(&tx).await {
            eprintln!("fund tx confirmed after {attempt} attempts");
            break;
        }
        eprintln!("attempt {attempt}: waiting for fund tx confirmation...");
    }

    wallet.handle_refresh_balance().await.unwrap();
    let acct = wallet.active_account().unwrap();
    assert!(
        acct.balance > 0,
        "Balance should be > 0 after funding on Sepolia"
    );
}

#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_transfer_and_rollover_sepolia() {
    let config = NetworkConfig::from_env();
    let rpc = RpcClient::new(&config).expect("RpcClient");

    // Fresh mnemonics for both sender and receiver.
    let mnemonic_a = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let mnemonic_b = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");

    // Wallet A (sender)
    let storage_a = Box::new(MockPlatformStorage::new());
    let mut wallet_a = WalletCore::new(storage_a, config.clone());
    wallet_a
        .handle_onboarding(&mnemonic_a)
        .await
        .expect("onboarding A failed");
    let addr_a = wallet_a.active_account().unwrap().starknet_address;

    // Wallet B (receiver)
    let storage_b = Box::new(MockPlatformStorage::new());
    let mut wallet_b = WalletCore::new(storage_b, config.clone());
    wallet_b
        .handle_onboarding(&mnemonic_b)
        .await
        .expect("onboarding B failed");

    // Faucet STRK to wallet A so it can do the ERC-20 approve inside fund().
    // fund("50") = 5 tongo units → approve(5 * rate)
    faucet_strk(&config, &addr_a, 5_000_000_000_000_000_000).await;

    // Fund A to get Tongo balance (enough for a transfer).
    let fund_tx = wallet_a.handle_fund("50").await.expect("fund A failed");
    eprintln!("fund A tx: {fund_tx}");

    // Wait for fund confirmation
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(&fund_tx).await {
            eprintln!("fund A confirmed after {attempt} attempts");
            break;
        }
        eprintln!("attempt {attempt}: waiting for fund A confirmation...");
    }

    wallet_a
        .handle_refresh_balance()
        .await
        .expect("refresh A balance failed");
    let acct_a = wallet_a.active_account().unwrap();
    assert!(
        acct_a.balance > 0,
        "wallet A should have Tongo balance after fund"
    );
    eprintln!("wallet A balance: {}", acct_a.balance);

    // Transfer A → B using B's public key
    let recipient_pk = wallet_b
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();
    wallet_a
        .handle_transfer_op("10", &recipient_pk)
        .await
        .expect("transfer A→B failed");

    // Wait for transfer to be confirmed, then check B's pending balance
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_b
            .handle_refresh_balance()
            .await
            .expect("refresh B balance failed");
        let acct_b = wallet_b.active_account().unwrap();
        if acct_b.pending > 0 {
            eprintln!(
                "wallet B pending confirmed after {attempt} attempts: {}",
                acct_b.pending
            );
            break;
        }
        eprintln!("attempt {attempt}: wallet B pending still 0, waiting...");
    }
    let acct_b = wallet_b.active_account().unwrap();
    assert!(
        acct_b.pending > 0,
        "wallet B should have pending balance after transfer"
    );

    wallet_b
        .handle_rollover_op()
        .await
        .expect("rollover B failed");

    // Wait for rollover confirmation
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_b
            .handle_refresh_balance()
            .await
            .expect("refresh B balance after rollover failed");
        let acct_b = wallet_b.active_account().unwrap();
        if acct_b.balance > 0 && acct_b.pending == 0 {
            eprintln!(
                "wallet B rollover confirmed after {attempt} attempts: balance={}",
                acct_b.balance
            );
            break;
        }
        eprintln!(
            "attempt {attempt}: wallet B balance={}, pending={}, waiting...",
            acct_b.balance, acct_b.pending
        );
    }
    let acct_b = wallet_b.active_account().unwrap();
    assert!(
        acct_b.balance > 0,
        "wallet B should have balance after rollover"
    );
    assert_eq!(
        acct_b.pending, 0,
        "wallet B pending should be 0 after rollover"
    );
}

#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_lazy_deploy_via_paymaster_sepolia() {
    // End-to-end test that a fresh Starknet account is deployed lazily on first fund()
    // using the AVNU paymaster (gasfree mode). Uses a newly generated mnemonic so the
    // counterfactual account is guaranteed to be undeployed.
    //
    // Env requirements:
    // - OUBLI_RPC_URL, OUBLI_CHAIN_ID, OUBLI_TONGO_CONTRACT, OUBLI_TOKEN_CONTRACT
    // - OUBLI_ACCOUNT_CLASS_HASH, OUBLI_PAYMASTER_URL
    // - OUBLI_TEST_MNEMONIC_A (faucet: pre-funded account that sends STRK to fresh account)
    let config = NetworkConfig::from_env();

    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());

    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");

    // Onboard — derives counterfactual Starknet address and stores it.
    wallet
        .handle_onboarding(&mnemonic)
        .await
        .expect("onboarding failed");

    let acct = wallet
        .active_account()
        .expect("wallet should have active account after onboarding");
    let starknet_addr = acct.starknet_address;

    // Before any operation, the counterfactual account at this address should NOT be deployed.
    let rpc = RpcClient::new(&config).expect("failed to create RpcClient");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid OUBLI_ACCOUNT_CLASS_HASH");
    let deployed_before = rpc
        .is_account_deployed(&starknet_addr, &class_hash)
        .await
        .expect("RPC getClassHashAt failed");
    assert!(
        !deployed_before,
        "expected account to be undeployed before first operation (generated mnemonic)"
    );

    // Seed the fresh account with STRK so the ERC-20 approve + Tongo deposit can succeed.
    // fund() requires the account to hold STRK for the approve(amount * rate) call.
    // The on-chain rate determines how much STRK is needed per tongo unit.
    // 1 STRK = 10^18 wei (18 decimals) — generous enough for a tiny fund("10").
    faucet_strk(&config, &starknet_addr, 1_000_000_000_000_000_000).await;

    // First fund operation should trigger lazy DEPLOY_ACCOUNT via the AVNU paymaster
    // (PaymasterSubmitter::ensure_deployed), then execute the fund call.
    let tx_hash = wallet
        .handle_fund("10")
        .await
        .expect("fund with lazy deploy failed");
    assert!(
        !tx_hash.is_empty(),
        "fund with lazy deploy should return a non-empty tx hash"
    );

    // Wait for the deploy+fund tx to be confirmed on-chain before checking.
    eprintln!("fund tx hash: {tx_hash}");
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(&tx_hash).await {
            eprintln!("fund tx confirmed after {attempt} attempts");
            break;
        }
        eprintln!("attempt {attempt}: waiting for fund tx confirmation...");
    }

    // After fund(), the account should now be deployed on-chain.
    let deployed_after = rpc
        .is_account_deployed(&starknet_addr, &class_hash)
        .await
        .expect("RPC getClassHashAt failed after fund");
    assert!(
        deployed_after,
        "expected account to be deployed on-chain after first fund()"
    );
}

#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_get_erc20_balance_sepolia() {
    let config = NetworkConfig::from_env();
    let rpc = RpcClient::new(&config).expect("RpcClient");
    let token_contract = Felt::from_hex(&config.token_contract).expect("invalid token_contract");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");

    // Fresh mnemonic — guaranteed unfunded.
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());
    wallet
        .handle_onboarding(&mnemonic)
        .await
        .expect("onboarding failed");
    let starknet_addr = wallet.active_account().unwrap().starknet_address;

    // Unfunded account should have zero STRK balance.
    let balance_before = rpc
        .get_erc20_balance(&token_contract, &starknet_addr)
        .await
        .expect("get_erc20_balance failed");
    assert_eq!(balance_before, 0, "fresh account should have 0 STRK");

    // Send 1 STRK from faucet.
    faucet_strk(&config, &starknet_addr, 1_000_000_000_000_000_000).await;

    // Balance should now be > 0.
    let balance_after = rpc
        .get_erc20_balance(&token_contract, &starknet_addr)
        .await
        .expect("get_erc20_balance failed after faucet");
    assert!(
        balance_after > 0,
        "STRK balance should be > 0 after faucet_strk"
    );
    eprintln!("STRK balance after faucet: {balance_after}");

    // --- Cleanup: route STRK back to faucet via auto-fund → ragequit ---
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    // First refresh → auto-fund deploys account
    wallet.handle_refresh_balance().await.ok();
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if rpc
            .is_account_deployed(&starknet_addr, &class_hash)
            .await
            .unwrap_or(false)
        {
            eprintln!("cleanup: deployed after {attempt} attempts");
            break;
        }
        eprintln!("cleanup attempt {attempt}: waiting for deploy...");
    }
    // Second refresh → auto-fund funds (STRK → Tongo)
    wallet.handle_refresh_balance().await.ok();
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet.handle_refresh_balance().await.ok();
        if wallet.active_account().unwrap().balance > 0 {
            eprintln!("cleanup: funded after {attempt} attempts");
            break;
        }
        eprintln!("cleanup attempt {attempt}: waiting for fund...");
    }
    // Ragequit everything back to faucet
    match wallet.handle_ragequit_op(&faucet_hex).await {
        Ok(tx) => eprintln!("cleanup: ragequit tx {tx}"),
        Err(e) => eprintln!("cleanup: ragequit failed (non-fatal): {e}"),
    }
}

#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_auto_fund_deploys_and_funds_sepolia() {
    let config = NetworkConfig::from_env();
    let rpc = RpcClient::new(&config).expect("RpcClient");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    // --- Phase 1: Auto-fund (deploy + fund) ---

    // Wallet A (sender)
    let mnemonic_a = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let storage_a = Box::new(MockPlatformStorage::new());
    let mut wallet_a = WalletCore::new(storage_a, config.clone());
    wallet_a
        .handle_onboarding(&mnemonic_a)
        .await
        .expect("onboarding A failed");
    let addr_a = wallet_a.active_account().unwrap().starknet_address;
    eprintln!("wallet A: {:#066x}", addr_a);

    // Wallet B (receiver)
    let mnemonic_b = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let storage_b = Box::new(MockPlatformStorage::new());
    let mut wallet_b = WalletCore::new(storage_b, config.clone());
    wallet_b
        .handle_onboarding(&mnemonic_b)
        .await
        .expect("onboarding B failed");
    eprintln!(
        "wallet B: {:#066x}",
        wallet_b.active_account().unwrap().starknet_address
    );

    // Faucet → A (STRK)
    faucet_strk(&config, &addr_a, 1_000_000_000_000_000_000).await;

    // Verify A is NOT deployed yet.
    assert!(
        !rpc.is_account_deployed(&addr_a, &class_hash)
            .await
            .expect("is_account_deployed failed"),
        "account should NOT be deployed before auto-fund"
    );

    // First refresh → auto-fund detects STRK, deploys account, returns early.
    wallet_a
        .handle_refresh_balance()
        .await
        .expect("first refresh failed");
    eprintln!("first refresh done — deploy submitted");

    // Wait for deploy confirmation.
    let mut deploy_confirmed = false;
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if rpc
            .is_account_deployed(&addr_a, &class_hash)
            .await
            .unwrap_or(false)
        {
            eprintln!("account deployed after {attempt} attempts");
            deploy_confirmed = true;
            break;
        }
        eprintln!("attempt {attempt}: waiting for deploy confirmation...");
    }
    assert!(
        deploy_confirmed,
        "account should be deployed after first auto-fund refresh"
    );

    // Second refresh → auto-fund detects STRK, account deployed, proceeds with fund.
    wallet_a
        .handle_refresh_balance()
        .await
        .expect("second refresh failed");
    eprintln!("second refresh done — fund submitted");

    // Wait for fund confirmation (Tongo balance > 0).
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_a
            .handle_refresh_balance()
            .await
            .expect("refresh during fund wait failed");
        let acct = wallet_a.active_account().unwrap();
        if acct.balance > 0 {
            eprintln!(
                "Tongo balance confirmed after {attempt} attempts: {}",
                acct.balance
            );
            break;
        }
        eprintln!("attempt {attempt}: balance still 0, waiting for fund confirmation...");
    }
    let balance_a = wallet_a.active_account().unwrap().balance;
    assert!(balance_a > 0, "Tongo balance should be > 0 after auto-fund");

    // --- Phase 2: Private transfer A → B ---

    let recipient_pk = wallet_b
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();
    wallet_a
        .handle_transfer_op("10", &recipient_pk)
        .await
        .expect("transfer A→B failed");
    eprintln!("transfer A→B submitted");

    // Wait for B to see pending balance (confirms the transfer tx on-chain).
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_b
            .handle_refresh_balance()
            .await
            .expect("refresh B failed");
        let acct_b = wallet_b.active_account().unwrap();
        if acct_b.pending > 0 {
            eprintln!(
                "wallet B pending confirmed after {attempt} attempts: {}",
                acct_b.pending
            );
            break;
        }
        eprintln!("attempt {attempt}: wallet B pending still 0, waiting...");
    }
    assert!(
        wallet_b.active_account().unwrap().pending > 0,
        "wallet B should have pending balance after transfer"
    );

    // --- Phase 3: Rollover B ---

    wallet_b
        .handle_rollover_op()
        .await
        .expect("rollover B failed");
    eprintln!("rollover B submitted");

    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_b
            .handle_refresh_balance()
            .await
            .expect("refresh B after rollover failed");
        let acct_b = wallet_b.active_account().unwrap();
        if acct_b.balance > 0 && acct_b.pending == 0 {
            eprintln!(
                "wallet B rollover confirmed after {attempt} attempts: balance={}",
                acct_b.balance
            );
            break;
        }
        eprintln!(
            "attempt {attempt}: wallet B balance={}, pending={}, waiting...",
            acct_b.balance, acct_b.pending
        );
    }
    assert!(
        wallet_b.active_account().unwrap().balance > 0,
        "wallet B should have balance after rollover"
    );

    // --- Phase 4: Return funds to faucet ---

    // Sync A's on-chain state after the transfer confirmed.
    wallet_a
        .handle_refresh_balance()
        .await
        .expect("refresh A post-transfer failed");

    // A ragequits remaining balance to faucet.
    if wallet_a.active_account().unwrap().balance > 0 {
        match wallet_a.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => eprintln!("wallet A ragequit tx: {tx}"),
            Err(e) => eprintln!("wallet A ragequit failed (non-fatal): {e}"),
        }
    }

    // B ragequits everything to faucet.
    match wallet_b.handle_ragequit_op(&faucet_hex).await {
        Ok(tx) => eprintln!("wallet B ragequit tx: {tx}"),
        Err(e) => eprintln!("wallet B ragequit failed (non-fatal): {e}"),
    }

    eprintln!("funds returned to faucet {faucet_hex}");
}

/// Helper: generate fresh mnemonics and print derived addresses.
/// Run with: cargo test -p oubli-wallet --test integration test_generate_fresh_mnemonics -- --ignored --nocapture
#[tokio::test]
#[ignore = "run manually to generate fresh mnemonics"]
async fn test_generate_fresh_mnemonics() {
    let config = networks::sepolia::config();

    for label in ["MNEMONIC", "MNEMONIC_A", "MNEMONIC_B"] {
        let mnemonic = krusty_kms::generate_mnemonic(24).expect("mnemonic generation");
        let storage = Box::new(MockPlatformStorage::new());
        let mut wallet = WalletCore::new(storage, config.clone());
        wallet.handle_onboarding(&mnemonic).await.unwrap();
        let addr = wallet.active_account().unwrap().starknet_address;
        eprintln!("{label}:");
        eprintln!("  mnemonic: {mnemonic}");
        eprintln!("  address:  {:#066x}", addr);
        eprintln!();
    }
}

/// Prints the faucet (OUBLI_TEST_MNEMONIC_A) Starknet address.
/// Fund this address with STRK on Sepolia so the faucet can seed fresh test accounts.
///
/// Run with: set -a && . crates/oubli-wallet/tests/sepolia.env && set +a && \
///   cargo test -p oubli-wallet --test integration test_print_faucet_address -- --ignored --nocapture
#[tokio::test]
#[ignore = "run manually to print faucet address"]
async fn test_print_faucet_address() {
    let config = NetworkConfig::from_env();

    let mnemonic_a = std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A");
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config);
    wallet.handle_onboarding(&mnemonic_a).await.unwrap();
    eprintln!(
        "OUBLI_TEST_MNEMONIC_A (faucet) — fund this Starknet address with STRK on Sepolia:\n  {:#066x}",
        wallet.active_account().unwrap().starknet_address
    );
}

/// Test that `get_activity()` returns empty for a brand-new wallet with no on-chain history.
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/sepolia.env && set +a && \
///   cargo test -p oubli-wallet --test integration test_get_activity_empty_sepolia -- --ignored --nocapture
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_get_activity_empty_sepolia() {
    let config = NetworkConfig::from_env();

    // Fresh mnemonic — never interacted with Tongo contract.
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");

    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config);
    wallet
        .handle_onboarding(&mnemonic)
        .await
        .expect("onboarding failed");

    let activity = wallet
        .get_activity()
        .await
        .expect("get_activity() should succeed even with no events");

    eprintln!("fresh wallet: got {} events (expected 0)", activity.len());
    assert!(
        activity.is_empty(),
        "fresh wallet with no on-chain history should have 0 events, got {}",
        activity.len()
    );
}

/// Test that `get_activity()` returns a Fund event after depositing into Tongo.
///
/// This is the full end-to-end test: fund → wait for confirmation → get_activity().
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/sepolia.env && set +a && \
///   cargo test -p oubli-wallet --test integration test_get_activity_after_fund_sepolia -- --ignored --nocapture
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_get_activity_after_fund_sepolia() {
    let config = NetworkConfig::from_env();
    let rpc = RpcClient::new(&config).expect("RpcClient");

    // Fresh mnemonic — guaranteed undeployed, zero balance.
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");

    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());
    wallet
        .handle_onboarding(&mnemonic)
        .await
        .expect("onboarding failed");

    let starknet_addr = wallet.active_account().unwrap().starknet_address;
    eprintln!("wallet address: {:#066x}", starknet_addr);

    // Before funding: activity should be empty.
    let activity_before = wallet
        .get_activity()
        .await
        .expect("get_activity before fund");
    assert!(
        activity_before.is_empty(),
        "fresh wallet should have 0 events before fund"
    );

    // Faucet sends STRK so the ERC-20 approve inside fund() can succeed.
    faucet_strk(&config, &starknet_addr, 1_000_000_000_000_000_000).await;

    // Fund: deploy + deposit via paymaster.
    let tx = wallet.handle_fund("10").await.expect("fund failed");
    assert!(!tx.is_empty());
    eprintln!("fund tx hash: {tx}");

    // Wait for confirmation.
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(&tx).await {
            eprintln!("fund tx confirmed after {attempt} attempts");
            break;
        }
        eprintln!("attempt {attempt}: waiting for fund tx confirmation...");
    }

    // After funding: activity should have at least one Fund event.
    let activity_after = wallet
        .get_activity()
        .await
        .expect("get_activity after fund");

    eprintln!("got {} activity events after fund:", activity_after.len());
    for (i, event) in activity_after.iter().enumerate() {
        eprintln!(
            "  [{i}] type={}, amount={:?}, tx={}...{}, block={}",
            event.event_type,
            event.amount_sats,
            &event.tx_hash[..10],
            &event.tx_hash[event.tx_hash.len().saturating_sub(6)..],
            event.block_number
        );
    }

    assert!(
        !activity_after.is_empty(),
        "wallet should have at least one event after fund"
    );

    // Verify the Fund event.
    let fund_event = activity_after
        .iter()
        .find(|e| e.event_type == "Fund")
        .expect("should have a Fund event");
    assert!(
        fund_event.amount_sats.is_some(),
        "Fund event should have an amount"
    );
    assert!(
        !fund_event.tx_hash.is_empty(),
        "tx_hash should not be empty"
    );
    assert!(fund_event.block_number > 0, "block_number should be > 0");

    // Verify event structure for all events.
    for event in &activity_after {
        assert_ne!(event.event_type, "BalanceDeclared");
        assert_ne!(event.event_type, "TransferDeclared");
        assert!(
            [
                "Fund",
                "Withdraw",
                "Ragequit",
                "Rollover",
                "TransferIn",
                "TransferOut"
            ]
            .contains(&event.event_type.as_str()),
            "unexpected event_type: {}",
            event.event_type
        );
    }
}

/// Test that `handle_send` with a Starknet address (withdraw) works end-to-end.
///
/// Flow: fresh wallet → faucet STRK → auto-fund (deploy + sweep into Tongo) →
///       send (withdraw) to faucet Starknet address → verify balance decreased
///       and faucet received STRK.
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/sepolia.env && set +a && \
///   cargo test -p oubli-wallet --test integration test_send_withdraw_to_starknet_sepolia -- --ignored --nocapture
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_send_withdraw_to_starknet_sepolia() {
    let config = NetworkConfig::from_env();
    let rpc = RpcClient::new(&config).expect("RpcClient");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let token_contract = Felt::from_hex(&config.token_contract).expect("invalid token contract");
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    // Fresh wallet
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());
    wallet
        .handle_onboarding(&mnemonic)
        .await
        .expect("onboarding failed");

    let starknet_addr = wallet.active_account().unwrap().starknet_address;
    eprintln!("wallet address: {:#066x}", starknet_addr);

    // Faucet sends STRK so auto-fund can deploy + sweep into Tongo.
    faucet_strk(&config, &starknet_addr, 2_000_000_000_000_000_000).await;

    // First refresh → auto-fund detects STRK, deploys account.
    wallet
        .handle_refresh_balance()
        .await
        .expect("first refresh failed");
    if let Some(err) = wallet.last_auto_fund_error() {
        eprintln!("auto-fund error after first refresh: {err}");
    }
    eprintln!("first refresh done — deploy submitted");

    // Wait for deploy confirmation.
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if rpc
            .is_account_deployed(&starknet_addr, &class_hash)
            .await
            .unwrap_or(false)
        {
            eprintln!("account deployed after {attempt} attempts");
            break;
        }
        // Re-trigger auto-fund in case deploy was rejected/expired
        if attempt % 6 == 0 {
            wallet.handle_refresh_balance().await.ok();
            if let Some(err) = wallet.last_auto_fund_error() {
                eprintln!("auto-fund error at attempt {attempt}: {err}");
            }
        }
        eprintln!("attempt {attempt}: waiting for deploy...");
    }

    // Second refresh → auto-fund detects STRK, account deployed, proceeds with fund.
    wallet
        .handle_refresh_balance()
        .await
        .expect("second refresh failed");
    if let Some(err) = wallet.last_auto_fund_error() {
        eprintln!("auto-fund error after second refresh: {err}");
    }
    eprintln!("second refresh done — fund submitted");

    // Wait for fund confirmation (Tongo balance > 0).
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet
            .handle_refresh_balance()
            .await
            .expect("refresh during fund wait failed");
        if let Some(err) = wallet.last_auto_fund_error() {
            eprintln!("auto-fund error at attempt {attempt}: {err}");
        }
        if wallet.active_account().unwrap().balance > 0 {
            eprintln!("Tongo balance confirmed after {attempt} attempts");
            break;
        }
        eprintln!("attempt {attempt}: waiting for fund...");
    }
    let balance_before = wallet.active_account().unwrap().balance;
    assert!(
        balance_before > 0,
        "should have Tongo balance after auto-fund"
    );
    eprintln!("Tongo balance before withdraw: {balance_before}");

    // Record faucet's STRK balance before withdraw
    let faucet_strk_before = rpc
        .get_erc20_balance(&token_contract, &faucet_addr)
        .await
        .expect("get faucet STRK balance");
    eprintln!("faucet STRK before: {faucet_strk_before}");

    // Send (withdraw) to faucet Starknet address via handle_send
    let tx = wallet
        .handle_send("10", &faucet_hex)
        .await
        .expect("handle_send (withdraw) failed");
    eprintln!("withdraw tx: {tx}");

    // Wait for tx confirmation
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(&tx).await {
            eprintln!("withdraw tx confirmed after {attempt} attempts");
            break;
        }
        eprintln!("attempt {attempt}: waiting for withdraw tx confirmation...");
    }

    // Verify sender's Tongo balance decreased
    wallet
        .handle_refresh_balance()
        .await
        .expect("refresh after withdraw");
    let balance_after = wallet.active_account().unwrap().balance;
    eprintln!("Tongo balance after withdraw: {balance_after}");
    assert!(
        balance_after < balance_before,
        "Tongo balance should decrease after withdraw (before={balance_before}, after={balance_after})"
    );

    // Verify faucet received STRK
    let faucet_strk_after = rpc
        .get_erc20_balance(&token_contract, &faucet_addr)
        .await
        .expect("get faucet STRK balance after");
    eprintln!("faucet STRK after: {faucet_strk_after}");
    assert!(
        faucet_strk_after > faucet_strk_before,
        "faucet STRK balance should increase after withdraw (before={faucet_strk_before}, after={faucet_strk_after})"
    );

    // Cleanup: ragequit remaining balance back to faucet
    wallet.handle_refresh_balance().await.ok();
    let remaining = wallet.active_account().unwrap().balance;
    if remaining > 0 {
        match wallet.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => eprintln!("cleanup ragequit tx: {tx}"),
            Err(e) => eprintln!("cleanup ragequit failed (non-fatal): {e}"),
        }
    }
}

/// Test that a fresh wallet receiving a Tongo transfer gets its pending balance
/// automatically rolled over into spendable balance via `handle_refresh_balance`
/// alone — no explicit `handle_rollover_op` call.
///
/// This is the "receive-first" UX: user creates wallet, someone sends them funds,
/// and the funds appear as spendable after a balance refresh.
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/sepolia.env && set +a && \
///   cargo test -p oubli-wallet --test integration test_auto_rollover_on_refresh_sepolia -- --ignored --nocapture
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_auto_rollover_on_refresh_sepolia() {
    let config = NetworkConfig::from_env();
    let rpc = RpcClient::new(&config).expect("RpcClient");
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    // Sender wallet: fund via faucet + auto-fund
    let mnemonic_a = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let storage_a = Box::new(MockPlatformStorage::new());
    let mut wallet_a = WalletCore::new(storage_a, config.clone());
    wallet_a
        .handle_onboarding(&mnemonic_a)
        .await
        .expect("onboarding A failed");
    let addr_a = wallet_a.active_account().unwrap().starknet_address;

    faucet_strk(&config, &addr_a, 2_000_000_000_000_000_000).await;

    // Wait for auto-fund to deploy + sweep into Tongo
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    wallet_a
        .handle_refresh_balance()
        .await
        .expect("first refresh A");
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if rpc
            .is_account_deployed(&addr_a, &class_hash)
            .await
            .unwrap_or(false)
        {
            eprintln!("wallet A deployed after {attempt} attempts");
            break;
        }
    }
    wallet_a
        .handle_refresh_balance()
        .await
        .expect("second refresh A");
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_a.handle_refresh_balance().await.ok();
        if wallet_a.active_account().unwrap().balance > 0 {
            eprintln!("wallet A funded after {attempt} attempts");
            break;
        }
    }
    assert!(
        wallet_a.active_account().unwrap().balance > 0,
        "wallet A should have Tongo balance"
    );

    // Receiver wallet: fresh, never funded, never deployed
    let mnemonic_b = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    let storage_b = Box::new(MockPlatformStorage::new());
    let mut wallet_b = WalletCore::new(storage_b, config.clone());
    wallet_b
        .handle_onboarding(&mnemonic_b)
        .await
        .expect("onboarding B failed");

    let b_pk = wallet_b
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();

    // Verify B starts with zero balance and zero pending
    assert_eq!(wallet_b.active_account().unwrap().balance, 0);
    assert_eq!(wallet_b.active_account().unwrap().pending, 0);

    // Transfer A → B
    let tx = wallet_a
        .handle_transfer_op("10", &b_pk)
        .await
        .expect("transfer A→B failed");
    eprintln!("transfer A→B tx: {tx}");

    // Wait for transfer to appear as pending on B — using refresh_balance ONLY
    // (no explicit handle_rollover_op)
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet_b
            .handle_refresh_balance()
            .await
            .expect("refresh B failed");
        let acct_b = wallet_b.active_account().unwrap();
        eprintln!(
            "attempt {attempt}: wallet B balance={}, pending={}",
            acct_b.balance, acct_b.pending
        );
        // Auto-rollover should move pending → balance within the refresh call
        if acct_b.balance > 0 && acct_b.pending == 0 {
            eprintln!("auto-rollover confirmed after {attempt} attempts");
            break;
        }
    }

    let acct_b = wallet_b.active_account().unwrap();
    assert!(
        acct_b.balance > 0,
        "wallet B should have spendable balance after auto-rollover (got balance={}, pending={})",
        acct_b.balance,
        acct_b.pending,
    );
    assert_eq!(
        acct_b.pending, 0,
        "wallet B pending should be 0 after auto-rollover"
    );

    // Cleanup: ragequit both wallets back to faucet
    wallet_a.handle_refresh_balance().await.ok();
    if wallet_a.active_account().unwrap().balance > 0 {
        match wallet_a.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => eprintln!("cleanup A ragequit tx: {tx}"),
            Err(e) => eprintln!("cleanup A ragequit failed (non-fatal): {e}"),
        }
    }
    if wallet_b.active_account().unwrap().balance > 0 {
        match wallet_b.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => eprintln!("cleanup B ragequit tx: {tx}"),
            Err(e) => eprintln!("cleanup B ragequit failed (non-fatal): {e}"),
        }
    }
}

/// Full-flow integration test against Starknet MAINNET.
///
/// WARNING: This uses REAL WBTC! Budget: ~10,000 sats (0.0001 WBTC).
///
/// Tests all wallet operations using 5 seeds (S1–S5) funded from a proxy faucet (S0).
/// Recovers most WBTC back to S0 at the end.
///
/// # Faucet architecture
///
/// The faucet (S0, `OUBLI_TEST_MNEMONIC_A`) distributes WBTC to test wallets via plain
/// ERC-20 transfers using `SingleOwnerAccount` (starknet-rs). This bypasses WalletCore
/// entirely, so the faucet's public WBTC balance is preserved.
///
/// **Do NOT create a `WalletCore` from the faucet mnemonic** unless deploying the account
/// for the first time. WalletCore's auto-fund will sweep all public WBTC into the Tongo
/// privacy pool, making it unavailable for ERC-20 distribution. If this happens
/// accidentally, use `test_recover_strk_mainnet` to ragequit the balance back to public.
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/mainnet.env && set +a
///   cargo test -p oubli-wallet --test mainnet_full_flow -- --ignored --nocapture
use oubli_store::MockPlatformStorage;
use oubli_wallet::config::NetworkConfig;
use oubli_wallet::core::WalletCore;
use oubli_wallet::rpc::RpcClient;
use serial_test::serial;
use starknet_types_core::felt::Felt;

/// Base unit for test distributions.
/// WBTC has 8 decimals; 1000 raw units = 1000 sats = 100 tongo units.
const TOKEN_UNIT: u128 = 1000;

// ── Helpers ───────────────────────────────────────────────────

/// Derive the faucet's Starknet address without triggering auto-fund.
fn faucet_starknet_address(config: &NetworkConfig) -> Felt {
    use krusty_kms_client::starknet_rust::core::types::Felt as RsFelt;
    use krusty_kms_client::starknet_rust::signers::SigningKey;

    let mnemonic_a =
        std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A");
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

/// Send WBTC from the faucet (OUBLI_TEST_MNEMONIC_A) to a target address via raw ERC-20 transfer.
async fn faucet_strk(config: &NetworkConfig, to_address: &Felt, amount: u128) {
    use krusty_kms_client::starknet_rust::accounts::{
        Account, ExecutionEncoding, SingleOwnerAccount,
    };
    use krusty_kms_client::starknet_rust::core::types::{
        BlockId, BlockTag, Call as RsCall, Felt as RsFelt,
    };
    use krusty_kms_client::starknet_rust::core::utils::get_selector_from_name;
    use krusty_kms_client::starknet_rust::signers::{LocalWallet, SigningKey};

    let mnemonic_a =
        std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A as faucet");

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

    let provider =
        krusty_kms_client::create_provider(&config.rpc_url).expect("create_provider failed");
    let signer = LocalWallet::from(signing_key);
    let chain_id = {
        let bytes = config.chain_id.as_bytes();
        let mut buf = [0u8; 32];
        let start = 32usize.saturating_sub(bytes.len());
        buf[start..].copy_from_slice(bytes);
        RsFelt::from_bytes_be(&buf)
    };
    let mut account = SingleOwnerAccount::new(
        provider,
        signer,
        address_rs,
        chain_id,
        ExecutionEncoding::New,
    );
    account.set_block_id(BlockId::Tag(BlockTag::Latest));

    let erc20 = RsFelt::from_hex(&config.token_contract).expect("invalid token contract");
    let recipient = RsFelt::from_bytes_be(&to_address.to_bytes_be());
    let call = RsCall {
        to: erc20,
        selector: get_selector_from_name("transfer").expect("selector"),
        calldata: vec![recipient, RsFelt::from(amount), RsFelt::ZERO],
    };

    let result = account
        .execute_v3(vec![call])
        .send()
        .await
        .expect("faucet transfer failed");
    let tx_hash = format!("{:#066x}", result.transaction_hash);
    eprintln!("  faucet_strk tx: {tx_hash}");

    wait_for_tx(config, &tx_hash).await;
}

/// Wait for a transaction to be confirmed on-chain (up to ~120s for mainnet).
async fn wait_for_tx(config: &NetworkConfig, tx_hash: &str) {
    let rpc = RpcClient::new(config).expect("RpcClient");
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Ok(true) = rpc.is_tx_confirmed(tx_hash).await {
            eprintln!("  tx confirmed after {attempt} attempts");
            return;
        }
    }
    panic!("tx {tx_hash} was not confirmed in time");
}

/// Wait for auto-fund to deploy account and sweep STRK into Tongo pool.
/// Returns the final Tongo balance.
async fn wait_for_auto_fund(
    wallet: &mut WalletCore,
    config: &NetworkConfig,
    label: &str,
) -> u128 {
    let rpc = RpcClient::new(config).expect("RpcClient");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let starknet_addr = wallet.active_account().unwrap().starknet_address;

    // First refresh → triggers deploy
    wallet.handle_refresh_balance().await.ok();

    // Wait for deploy
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if rpc
            .is_account_deployed(&starknet_addr, &class_hash)
            .await
            .unwrap_or(false)
        {
            eprintln!("  {label}: deployed after {attempt} attempts");
            break;
        }
    }

    // Second refresh → triggers fund
    wallet.handle_refresh_balance().await.ok();

    // Wait for Tongo balance > 0
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet.handle_refresh_balance().await.ok();
        let balance = wallet.active_account().unwrap().balance;
        if balance > 0 {
            eprintln!("  {label}: funded after {attempt} attempts, balance={balance}");
            return balance;
        }
    }
    panic!("{label}: auto-fund did not complete in time");
}

/// Wait for rollover to complete (pending→0, balance increased).
async fn wait_for_rollover(wallet: &mut WalletCore, label: &str) -> u128 {
    for _attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet.handle_refresh_balance().await.ok();
        let acct = wallet.active_account().unwrap();
        if acct.balance > 0 && acct.pending == 0 {
            eprintln!("  {label}: rollover done, balance={}", acct.balance);
            return acct.balance;
        }
    }
    panic!("{label}: rollover did not complete in time");
}

/// Zero-pad a public key hex string to exactly 128 chars (uncompressed point).
fn pad_pubkey(hex: &str) -> String {
    let stripped = hex.strip_prefix("0x").unwrap_or(hex);
    format!("{:0>128}", stripped)
}

/// Wait for ragequit to be reflected (balance → 0) after tx confirmation.
async fn wait_for_ragequit_settled(wallet: &mut WalletCore, label: &str) {
    for attempt in 1..=12 {
        wallet.handle_refresh_balance().await.ok();
        let acct = wallet.active_account().unwrap();
        if acct.balance == 0 && acct.pending == 0 {
            eprintln!("  {label}: ragequit settled after {attempt} refresh(es)");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    let acct = wallet.active_account().unwrap();
    panic!("{label}: ragequit not settled, balance={}, pending={}", acct.balance, acct.pending);
}

/// Retry a paymaster operation up to `max_retries` times with increasing delays.
/// The AVNU paymaster intermittently rejects transactions with "execution call was rejected"
/// when on-chain state hasn't fully settled from a previous operation.
async fn retry_paymaster_op(
    wallet: &mut WalletCore,
    label: &str,
    max_retries: u32,
    op_name: &str,
    amount: &str,
    target: &str,
) -> String {
    for attempt in 1..=max_retries {
        let result = match op_name {
            "ragequit" => wallet.handle_ragequit_op(target).await,
            "withdraw" => wallet.handle_withdraw_op(amount, target).await,
            "fund" => wallet.handle_fund(amount).await,
            "transfer" => wallet.handle_transfer_op(amount, target).await,
            _ => panic!("unknown op: {op_name}"),
        };
        match result {
            Ok(tx) => return tx,
            Err(e) => {
                let msg = e.to_string();
                if attempt < max_retries && msg.contains("rejected") {
                    eprintln!("  {label}: {op_name} attempt {attempt}/{max_retries} rejected, retrying after delay...");
                    tokio::time::sleep(std::time::Duration::from_secs(10 * attempt as u64)).await;
                    wallet.handle_refresh_balance().await.ok();
                } else {
                    panic!("{label}: {op_name} failed after {attempt} attempts: {e}");
                }
            }
        }
    }
    unreachable!()
}

/// Create a wallet from a mnemonic.
async fn make_wallet(mnemonic: &str, config: &NetworkConfig, label: &str) -> WalletCore {
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());
    wallet
        .handle_onboarding(mnemonic)
        .await
        .unwrap_or_else(|e| panic!("{label}: onboarding failed: {e}"));
    let addr = wallet.active_account().unwrap().starknet_address;
    eprintln!("  {label}: {:#066x}", addr);
    wallet
}

// ── Main test ─────────────────────────────────────────────────

#[tokio::test]
#[serial]
#[ignore = "requires MAINNET env (source mainnet.env) — uses REAL WBTC"]
async fn test_full_flow_mainnet() {
    let config = NetworkConfig::from_env();
    assert_eq!(config.chain_id, "SN_MAIN", "this test must run against mainnet");

    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);
    eprintln!("=== Faucet (S0): {faucet_hex}");

    // ── Phase 0: Deploy faucet if needed ─────────────────────
    // On a fresh mainnet address the OZ account isn't deployed yet.
    // Deploy via WalletCore (paymaster), then ragequit to recover public STRK.
    let rpc_check = RpcClient::new(&config).expect("RpcClient");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let faucet_deployed = rpc_check
        .is_account_deployed(&faucet_addr, &class_hash)
        .await
        .unwrap_or(false);

    if !faucet_deployed {
        eprintln!("\n=== Phase 0: Deploying faucet account via paymaster");
        let faucet_mnemonic =
            std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A");
        let mut faucet_wallet = make_wallet(&faucet_mnemonic, &config, "S0 Faucet").await;

        // Auto-fund will deploy + sweep STRK into Tongo
        let faucet_tongo_bal = wait_for_auto_fund(&mut faucet_wallet, &config, "S0").await;
        eprintln!("  Faucet Tongo balance after auto-fund: {faucet_tongo_bal}");

        // Ragequit everything back to faucet's own Starknet address (public STRK)
        eprintln!("  Ragequit to recover public STRK...");
        let tx = faucet_wallet
            .handle_ragequit_op(&faucet_hex)
            .await
            .expect("faucet ragequit failed");
        eprintln!("  Faucet ragequit tx: {tx}");
        wait_for_tx(&config, &tx).await;
        eprintln!("  Faucet deployed and STRK recovered");
    } else {
        eprintln!("  Faucet already deployed");
    }

    // ── Generate seeds ───────────────────────────────────────
    // Print mnemonics so STRK can be recovered if the test fails mid-run.
    let m1 = krusty_kms::generate_mnemonic(12).unwrap();
    let m2 = krusty_kms::generate_mnemonic(12).unwrap();
    let m3 = krusty_kms::generate_mnemonic(12).unwrap();
    let m4 = krusty_kms::generate_mnemonic(12).unwrap();
    let m5 = krusty_kms::generate_mnemonic(12).unwrap();
    eprintln!("\n=== RECOVERY MNEMONICS (save these in case of failure):");
    eprintln!("  S1: {m1}");
    eprintln!("  S2: {m2}");
    eprintln!("  S3: {m3}");
    eprintln!("  S4: {m4}");
    eprintln!("  S5: {m5}");

    eprintln!("\n=== Phase 1: Setup wallets");
    let mut s1 = make_wallet(&m1, &config, "S1 Alice").await;
    let mut s2 = make_wallet(&m2, &config, "S2 Bob").await;
    let mut s3 = make_wallet(&m3, &config, "S3 Charlie").await;
    let mut s4 = make_wallet(&m4, &config, "S4 Diana").await;
    let mut s5 = make_wallet(&m5, &config, "S5 Eve").await;

    let s1_addr = s1.active_account().unwrap().starknet_address;
    let s2_addr = s2.active_account().unwrap().starknet_address;
    let s3_addr = s3.active_account().unwrap().starknet_address;
    let s4_addr = s4.active_account().unwrap().starknet_address;
    let s5_addr = s5.active_account().unwrap().starknet_address;

    // ── Phase 1: Distribute WBTC from faucet ─────────────────
    // Distribute sequentially with delays — mainnet RPC can lag on nonce updates.
    eprintln!("\n=== Phase 1: Distribute WBTC from faucet");
    let half = TOKEN_UNIT / 2;
    eprintln!("  S0 → S1 (2000 sats)");
    faucet_strk(&config, &s1_addr, 2 * TOKEN_UNIT).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    eprintln!("  S0 → S2 (500 sats)");
    faucet_strk(&config, &s2_addr, half).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    eprintln!("  S0 → S3 (1000 sats)");
    faucet_strk(&config, &s3_addr, TOKEN_UNIT).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    eprintln!("  S0 → S4 (1000 sats)");
    faucet_strk(&config, &s4_addr, TOKEN_UNIT).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    eprintln!("  S0 → S5 (500 sats)");
    faucet_strk(&config, &s5_addr, half).await;

    // ── Phase 2: Auto-fund (deploy + sweep WBTC → Tongo) ────
    eprintln!("\n=== Phase 2: Auto-fund (lazy deploy + fund)");
    let s1_bal = wait_for_auto_fund(&mut s1, &config, "S1").await;
    assert!(s1_bal > 0, "S1 should have Tongo balance after auto-fund");

    let s2_bal = wait_for_auto_fund(&mut s2, &config, "S2").await;
    assert!(s2_bal > 0, "S2 should have Tongo balance after auto-fund");

    let s3_bal = wait_for_auto_fund(&mut s3, &config, "S3").await;
    assert!(s3_bal > 0, "S3 should have Tongo balance after auto-fund");

    let s4_bal = wait_for_auto_fund(&mut s4, &config, "S4").await;
    assert!(s4_bal > 0, "S4 should have Tongo balance after auto-fund");

    let s5_bal = wait_for_auto_fund(&mut s5, &config, "S5").await;
    assert!(s5_bal > 0, "S5 should have Tongo balance after auto-fund");

    eprintln!("  Balances: S1={s1_bal} S2={s2_bal} S3={s3_bal} S4={s4_bal} S5={s5_bal}");

    // ── Phase 3A: Explicit Fund ──────────────────────────────
    eprintln!("\n=== Phase 3A: Explicit fund (S1)");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    faucet_strk(&config, &s1_addr, TOKEN_UNIT / 2).await;

    // Do NOT call handle_refresh_balance here — it triggers auto-fund
    // which sweeps tokens before the explicit fund can use them.
    let fund_tx = retry_paymaster_op(&mut s1, "S1", 3, "fund", "10", "").await;
    eprintln!("  S1 fund tx: {fund_tx}");
    wait_for_tx(&config, &fund_tx).await;

    // Refresh triggers auto-fund of remaining STRK. Wait for it to settle
    // so the on-chain cipher balance is consistent before transfers.
    s1.handle_refresh_balance().await.unwrap();
    for att in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s1.handle_refresh_balance().await.ok();
        let acct = s1.active_account().unwrap();
        if acct.balance > s1_bal && s1.last_auto_fund_error().is_none() {
            eprintln!("  S1 auto-fund settled after {att} attempts");
            break;
        }
    }
    let s1_bal_after_fund = s1.active_account().unwrap().balance;
    eprintln!("  S1 balance after explicit fund + auto-fund: {s1_bal_after_fund}");
    assert!(
        s1_bal_after_fund > s1_bal,
        "S1 balance should increase after explicit fund"
    );

    // ── Phase 3B: Transfers ──────────────────────────────────
    eprintln!("\n=== Phase 3B: Transfers");

    let s2_pk = pad_pubkey(&s2.active_account().unwrap().owner_public_key_hex);
    let s5_pk = pad_pubkey(&s5.active_account().unwrap().owner_public_key_hex);

    // Allow state to settle after auto-fund before starting transfers
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    s1.handle_refresh_balance().await.ok();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    eprintln!("  S1 → S2 transfer");
    let tx = retry_paymaster_op(&mut s1, "S1→S2", 3, "transfer", "10", &s2_pk).await;
    eprintln!("  S1→S2 tx: {tx}");

    // S1 → S5: wait for S1→S2 to confirm first (nonce + cipher sync)
    wait_for_tx(&config, &tx).await;
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    s1.handle_refresh_balance().await.ok();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    eprintln!("  S1 → S5 transfer");
    let tx = retry_paymaster_op(&mut s1, "S1→S5", 3, "transfer", "10", &s5_pk).await;
    eprintln!("  S1→S5 tx: {tx}");

    // S3 → S5 (transfer from different sender — independent nonce, no wait needed)
    eprintln!("  S3 → S5 transfer");
    let s3_tx = retry_paymaster_op(&mut s3, "S3→S5", 3, "transfer", "10", &s5_pk).await;
    eprintln!("  S3→S5 tx: {s3_tx}");

    // Wait for all transfers to confirm before rollovers
    wait_for_tx(&config, &tx).await; // S1→S5
    wait_for_tx(&config, &s3_tx).await; // S3→S5
    s1.handle_refresh_balance().await.ok();

    // ── Phase 3C: Rollovers ──────────────────────────────────
    eprintln!("\n=== Phase 3C: Rollovers");

    // Auto-rollover handles it via handle_refresh_balance
    eprintln!("  Waiting for S2 rollover (auto)...");
    let s2_bal_after_rollover = wait_for_rollover(&mut s2, "S2").await;
    assert!(s2_bal_after_rollover > s2_bal, "S2 balance should increase after rollover");

    eprintln!("  Waiting for S5 rollover (auto)...");
    let s5_bal_after_rollover = wait_for_rollover(&mut s5, "S5").await;
    assert!(
        s5_bal_after_rollover > s5_bal,
        "S5 balance should increase after rollover"
    );

    // ── Phase 3D: Transfer rolled-over funds (confirm spendable) ─
    eprintln!("\n=== Phase 3D: Transfer rolled-over funds (S2 → S1)");
    let s1_pk = pad_pubkey(&s1.active_account().unwrap().owner_public_key_hex);
    let tx = retry_paymaster_op(&mut s2, "S2→S1", 3, "transfer", "10", &s1_pk).await;
    eprintln!("  S2→S1 tx: {tx}");

    // S1 auto-rollover picks up pending automatically
    let s1_bal_before = s1.active_account().unwrap().balance;
    eprintln!("  Waiting for S1 balance to increase (auto-rollover)...");
    wait_for_tx(&config, &tx).await;
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s1.handle_refresh_balance().await.ok();
        let bal = s1.active_account().unwrap().balance;
        if bal > s1_bal_before {
            eprintln!("  S1: balance increased {s1_bal_before} → {bal} after {attempt} attempts");
            break;
        }
        assert!(attempt < 36, "S1 balance never increased after S2→S1 transfer");
    }

    // ── Phase 3E: Withdraw ───────────────────────────────────
    eprintln!("\n=== Phase 3E: Withdraw");

    s3.handle_refresh_balance().await.ok();
    let s3_bal = s3.active_account().unwrap().balance;
    eprintln!("  S3 balance before withdraw: {s3_bal}");

    eprintln!("  S3 withdraw → faucet");
    let tx = retry_paymaster_op(&mut s3, "S3", 3, "withdraw", "10", &faucet_hex).await;
    eprintln!("  S3 withdraw tx: {tx}");
    wait_for_tx(&config, &tx).await;

    // Wait for withdraw to be reflected on-chain
    let mut s3_bal_after = s3_bal;
    for attempt in 1..=12 {
        s3.handle_refresh_balance().await.ok();
        s3_bal_after = s3.active_account().unwrap().balance;
        if s3_bal_after < s3_bal {
            eprintln!("  S3 balance after withdraw: {s3_bal_after} (settled after {attempt} refresh)");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    assert!(s3_bal_after < s3_bal, "S3 balance should decrease after withdraw");

    // ── Phase 3F: Ragequit ───────────────────────────────────
    eprintln!("\n=== Phase 3F: Ragequit");

    // Let state settle after previous withdraw before attempting ragequit
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Ragequit to faucet (not self) to avoid auto-fund re-sweep cycle
    s4.handle_refresh_balance().await.ok();
    let s4_bal = s4.active_account().unwrap().balance;
    eprintln!("  S4 balance before ragequit: {s4_bal}");

    eprintln!("  S4 ragequit → faucet");
    let tx = retry_paymaster_op(&mut s4, "S4", 3, "ragequit", "", &faucet_hex).await;
    eprintln!("  S4 ragequit tx: {tx}");
    wait_for_tx(&config, &tx).await;

    wait_for_ragequit_settled(&mut s4, "S4").await;
    eprintln!("  S4 Tongo balance=0, pending=0 — ragequit confirmed");

    // ── Phase 4: Edge cases ──────────────────────────────────
    eprintln!("\n=== Phase 4: Edge cases");

    // 4.1: S5 ragequit (after rollover of multi-sender transfers)
    // Ragequit to faucet (not self) to avoid auto-fund re-sweep cycle
    eprintln!("  4.1: S5 ragequit → faucet (after multi-sender rollover)");
    s5.handle_refresh_balance().await.ok();
    let tx = retry_paymaster_op(&mut s5, "S5", 3, "ragequit", "", &faucet_hex).await;
    eprintln!("  S5 ragequit tx: {tx}");
    wait_for_tx(&config, &tx).await;

    wait_for_ragequit_settled(&mut s5, "S5").await;
    eprintln!("  S5 ragequit confirmed");

    // 4.2: Rollover with pending == 0 should error
    eprintln!("  4.2: S2 rollover with pending=0 (should fail)");
    s2.handle_refresh_balance().await.ok();
    let result = s2.handle_rollover_op().await;
    assert!(result.is_err(), "rollover with pending=0 should fail");
    let err = result.unwrap_err().to_string();
    eprintln!("  Expected error: {err}");
    assert!(err.contains("pending"), "error should mention pending");

    // 4.3: S4 re-fund after ragequit (wallet can be re-used)
    eprintln!("  4.3: S4 re-fund after ragequit");
    faucet_strk(&config, &s4_addr, TOKEN_UNIT / 2).await;
    let mut s4_settled = false;
    for att in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s4.handle_refresh_balance().await.ok();
        let bal = s4.active_account().unwrap().balance;
        let no_err = s4.last_auto_fund_error().is_none();
        if bal > 0 && no_err {
            // Wait for multiple blocks so the on-chain cipher balance fully settles.
            // Mainnet block times can be slower than Sepolia.
            for _ in 0..3 {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                s4.handle_refresh_balance().await.ok();
            }
            eprintln!("  S4 re-funded and settled after {att} attempts");
            s4_settled = true;
            break;
        }
    }
    assert!(s4_settled, "S4 should have settled after re-fund");
    let s4_refund_bal = s4.active_account().unwrap().balance;
    assert!(s4_refund_bal > 0, "S4 should have balance after re-fund");
    eprintln!("  S4 re-funded balance: {s4_refund_bal}");

    // 4.4: S4 ragequit again (confirm re-entry/re-exit works)
    eprintln!("  4.4: S4 second ragequit → faucet");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    s4.handle_refresh_balance().await.ok();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let tx = retry_paymaster_op(&mut s4, "S4-2nd", 3, "ragequit", "", &faucet_hex).await;
    eprintln!("  S4 second ragequit tx: {tx}");
    wait_for_tx(&config, &tx).await;
    wait_for_ragequit_settled(&mut s4, "S4 (2nd)").await;
    eprintln!("  S4 second ragequit confirmed");

    // ── Phase 5: Activity verification ───────────────────────
    eprintln!("\n=== Phase 5: Activity verification");

    let mut s1_activity = Vec::new();
    for attempt in 1..=3 {
        match s1.get_activity().await {
            Ok(a) => { s1_activity = a; break; }
            Err(e) => {
                eprintln!("  S1 get_activity attempt {attempt} failed: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
    eprintln!("  S1 activity: {} events", s1_activity.len());
    for (i, ev) in s1_activity.iter().enumerate() {
        eprintln!("    [{i}] type={} amount={:?} block={}", ev.event_type, ev.amount_sats, ev.block_number);
    }
    assert!(!s1_activity.is_empty(), "S1 should have activity events");
    let fund_events: Vec<_> = s1_activity.iter().filter(|e| e.event_type == "Fund").collect();
    assert!(!fund_events.is_empty(), "S1 should have Fund events");
    let transfer_out: Vec<_> = s1_activity.iter().filter(|e| e.event_type == "TransferOut").collect();
    assert!(!transfer_out.is_empty(), "S1 should have TransferOut events");
    let transfer_in: Vec<_> = s1_activity.iter().filter(|e| e.event_type == "TransferIn").collect();
    assert!(!transfer_in.is_empty(), "S1 should have TransferIn events");
    let rollover_events: Vec<_> = s1_activity.iter().filter(|e| e.event_type == "Rollover").collect();
    assert!(!rollover_events.is_empty(), "S1 should have Rollover events");

    // Retry S4 activity — mainnet RPC can be flaky
    let mut s4_activity = Vec::new();
    for attempt in 1..=3 {
        match s4.get_activity().await {
            Ok(a) => { s4_activity = a; break; }
            Err(e) => {
                eprintln!("  S4 get_activity attempt {attempt} failed: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
    eprintln!("  S4 activity: {} events", s4_activity.len());
    for (i, ev) in s4_activity.iter().enumerate() {
        eprintln!("    [{i}] type={} amount={:?} block={}", ev.event_type, ev.amount_sats, ev.block_number);
    }
    let ragequit_events: Vec<_> = s4_activity.iter().filter(|e| e.event_type == "Ragequit").collect();
    assert!(ragequit_events.len() >= 2, "S4 should have at least 2 Ragequit events");

    // ── Phase 6: WBTC recovery ───────────────────────────────
    eprintln!("\n=== Phase 6: WBTC recovery to faucet");

    // S1: ragequit remaining Tongo balance to faucet
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    s1.handle_refresh_balance().await.ok();
    let s1_remaining = s1.active_account().unwrap().balance;
    if s1_remaining > 0 {
        eprintln!("  S1 ragequit remaining ({s1_remaining}) → faucet");
        match s1.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => {
                eprintln!("  S1 ragequit tx: {tx}");
                wait_for_tx(&config, &tx).await;
            }
            Err(e) => eprintln!("  S1 ragequit failed (non-fatal): {e}"),
        }
    }

    // S2: ragequit remaining to faucet
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    s2.handle_refresh_balance().await.ok();
    let s2_remaining = s2.active_account().unwrap().balance;
    if s2_remaining > 0 {
        eprintln!("  S2 ragequit remaining ({s2_remaining}) → faucet");
        match s2.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => {
                eprintln!("  S2 ragequit tx: {tx}");
                wait_for_tx(&config, &tx).await;
            }
            Err(e) => eprintln!("  S2 ragequit failed (non-fatal): {e}"),
        }
    }

    // S3: ragequit remaining to faucet
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    s3.handle_refresh_balance().await.ok();
    let s3_remaining = s3.active_account().unwrap().balance;
    if s3_remaining > 0 {
        eprintln!("  S3 ragequit remaining ({s3_remaining}) → faucet");
        match s3.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => {
                eprintln!("  S3 ragequit tx: {tx}");
                wait_for_tx(&config, &tx).await;
            }
            Err(e) => eprintln!("  S3 ragequit failed (non-fatal): {e}"),
        }
    }

    // S4: already ragequit, return public WBTC via send_token
    s4.handle_refresh_balance().await.ok();
    eprintln!("  S4 send_token (public STRK from ragequit) → faucet");
    let token = Felt::from_hex(&config.token_contract).unwrap();
    let rpc = RpcClient::new(&config).unwrap();
    let s4_public = rpc
        .get_erc20_balance(&token, &s4_addr)
        .await
        .unwrap_or(0);
    if s4_public > TOKEN_UNIT / 10 {
        let send_amount = s4_public.saturating_sub(TOKEN_UNIT / 100);
        match s4.send_token(&faucet_addr, send_amount).await {
            Ok(tx) => eprintln!("  S4 send_token tx: {tx}"),
            Err(e) => eprintln!("  S4 send_token failed (non-fatal): {e}"),
        }
    }

    // S5: same as S4
    s5.handle_refresh_balance().await.ok();
    eprintln!("  S5 send_token (public STRK from ragequit) → faucet");
    let s5_public = rpc
        .get_erc20_balance(&token, &s5_addr)
        .await
        .unwrap_or(0);
    if s5_public > TOKEN_UNIT / 10 {
        let send_amount = s5_public.saturating_sub(TOKEN_UNIT / 100);
        match s5.send_token(&faucet_addr, send_amount).await {
            Ok(tx) => eprintln!("  S5 send_token tx: {tx}"),
            Err(e) => eprintln!("  S5 send_token failed (non-fatal): {e}"),
        }
    }

    eprintln!("\n=== DONE: Mainnet full flow test complete, WBTC recovered to faucet");
}

/// Test that `handle_send` with a Starknet address (withdraw from Tongo to L1 address)
/// works end-to-end on mainnet.
///
/// Flow: fresh wallet → faucet STRK → auto-fund (deploy + sweep into Tongo) →
///       handle_send (withdraw) to faucet Starknet address → verify balance decreased
///       and faucet received STRK.
///
/// WARNING: Uses REAL WBTC! Budget: ~2000 sats (recovered at the end).
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/mainnet.env && set +a
///   cargo test -p oubli-wallet --test mainnet_full_flow test_send_withdraw_to_starknet_mainnet -- --ignored --nocapture
#[tokio::test]
#[serial]
#[ignore = "requires MAINNET env (source mainnet.env) — uses REAL WBTC"]
async fn test_send_withdraw_to_starknet_mainnet() {
    let config = NetworkConfig::from_env();
    assert_eq!(config.chain_id, "SN_MAIN", "this test must run against mainnet");

    let rpc = RpcClient::new(&config).expect("RpcClient");
    let token_contract =
        Felt::from_hex(&config.token_contract).expect("invalid token contract");
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    // Fresh wallet
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    eprintln!("=== RECOVERY MNEMONIC (save in case of failure): {mnemonic}");
    let mut wallet = make_wallet(&mnemonic, &config, "Sender").await;

    let starknet_addr = wallet.active_account().unwrap().starknet_address;

    // Faucet sends WBTC so auto-fund can deploy + sweep into Tongo.
    eprintln!("  Faucet → Sender (2000 sats)");
    faucet_strk(&config, &starknet_addr, 2 * TOKEN_UNIT).await;

    // Auto-fund: deploy + sweep STRK into Tongo pool
    eprintln!("  Waiting for auto-fund...");
    let balance_before = wait_for_auto_fund(&mut wallet, &config, "Sender").await;
    assert!(balance_before > 0, "should have Tongo balance after auto-fund");
    eprintln!("  Tongo balance before withdraw: {balance_before}");

    // Record faucet's STRK balance before withdraw
    let faucet_strk_before = rpc
        .get_erc20_balance(&token_contract, &faucet_addr)
        .await
        .expect("get faucet STRK balance");
    eprintln!("  Faucet STRK before: {faucet_strk_before}");

    // Send (withdraw) to faucet Starknet address via handle_send
    eprintln!("  handle_send (withdraw) → faucet");
    let tx = retry_paymaster_op(&mut wallet, "Sender", 3, "withdraw", "10", &faucet_hex).await;
    eprintln!("  withdraw tx: {tx}");
    wait_for_tx(&config, &tx).await;

    // Verify sender's Tongo balance decreased
    wallet
        .handle_refresh_balance()
        .await
        .expect("refresh after withdraw");
    let balance_after = wallet.active_account().unwrap().balance;
    eprintln!("  Tongo balance after withdraw: {balance_after}");
    assert!(
        balance_after < balance_before,
        "Tongo balance should decrease after withdraw (before={balance_before}, after={balance_after})"
    );

    // Verify faucet received STRK
    let faucet_strk_after = rpc
        .get_erc20_balance(&token_contract, &faucet_addr)
        .await
        .expect("get faucet STRK balance after");
    eprintln!("  Faucet STRK after: {faucet_strk_after}");
    assert!(
        faucet_strk_after > faucet_strk_before,
        "faucet STRK balance should increase after withdraw (before={faucet_strk_before}, after={faucet_strk_after})"
    );

    // Cleanup: ragequit remaining balance back to faucet
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    wallet.handle_refresh_balance().await.ok();
    let remaining = wallet.active_account().unwrap().balance;
    if remaining > 0 {
        eprintln!("  Cleanup: ragequit remaining ({remaining}) → faucet");
        match wallet.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => {
                eprintln!("  cleanup ragequit tx: {tx}");
                wait_for_tx(&config, &tx).await;
            }
            Err(e) => eprintln!("  cleanup ragequit failed (non-fatal): {e}"),
        }
    }

    // Return public STRK from ragequit
    wallet.handle_refresh_balance().await.ok();
    let sender_public = rpc
        .get_erc20_balance(&token_contract, &starknet_addr)
        .await
        .unwrap_or(0);
    if sender_public > TOKEN_UNIT / 10 {
        let send_amount = sender_public.saturating_sub(TOKEN_UNIT / 100);
        match wallet.send_token(&faucet_addr, send_amount).await {
            Ok(tx) => eprintln!("  cleanup send_token tx: {tx}"),
            Err(e) => eprintln!("  cleanup send_token failed (non-fatal): {e}"),
        }
    }

    eprintln!("=== DONE: Mainnet withdraw test complete");
}

/// Debug: Query get_state for a fresh (unregistered) account to check what the contract returns.
/// Also test the full fetch_and_decrypt + auto_fund flow to diagnose "Proof Of Ownership failed".
#[tokio::test]
#[ignore = "run manually"]
async fn test_get_state_fresh_account() {
    let config = NetworkConfig::from_env();
    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    eprintln!("Mnemonic: {mnemonic}");
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config.clone());
    wallet.handle_onboarding(&mnemonic).await.unwrap();

    let acct = wallet.active_account().unwrap();
    let pk = &acct.tongo_account.keypair.public_key;
    let rpc = RpcClient::new(&config).expect("RpcClient");

    eprintln!("Querying get_state for fresh account...");
    match rpc.contract().get_state(pk).await {
        Ok(state) => {
            eprintln!("get_state SUCCEEDED:");
            eprintln!("  nonce: {}", state.nonce);
            eprintln!("  balance.l z={}", state.balance.l.z());
            eprintln!("  balance.r z={}", state.balance.r.z());
            eprintln!("  pending.l z={}", state.pending.l.z());
            eprintln!("  pending.r z={}", state.pending.r.z());
            eprintln!("  balance.l is_identity={}", state.balance.l.is_identity());
        }
        Err(e) => {
            eprintln!("get_state FAILED: {e}");
        }
    }

    // Test the full decrypt flow
    eprintln!("\nTesting fetch_and_decrypt_balance...");
    match wallet.handle_refresh_balance().await {
        Ok(()) => {
            let a = wallet.active_account().unwrap();
            eprintln!("  balance={}, pending={}, nonce={}", a.balance, a.pending, a.nonce);
            eprintln!("  cipher_balance is_some={}", a.cipher_balance.is_some());
        }
        Err(e) => {
            eprintln!("  fetch_and_decrypt_balance FAILED: {e}");
        }
    }

    // Check last auto fund error
    if let Some(err) = wallet.last_auto_fund_error() {
        eprintln!("  last_auto_fund_error: {err}");
    }

    // Check auditor key
    eprintln!("\nQuerying auditor_key...");
    match rpc.contract().auditor_key().await {
        Ok(Some(key)) => eprintln!("  auditor_key: Some (x={}, y={})", key.x(), key.y()),
        Ok(None) => eprintln!("  auditor_key: None"),
        Err(e) => eprintln!("  auditor_key query failed: {e}"),
    }
}

/// Reproduce the mobile auto-fund-on-login flow:
/// 1. Create fresh wallet
/// 2. Send WBTC to it
/// 3. Login (onboarding) → triggers deploy
/// 4. Wait for deploy
/// 5. Re-login → triggers fund
/// This tests the exact path that causes "Proof Of Ownership failed".
#[tokio::test]
#[serial]
#[ignore = "requires MAINNET env — uses REAL WBTC"]
async fn test_auto_fund_on_login_flow() {
    let config = NetworkConfig::from_env();
    assert_eq!(config.chain_id, "SN_MAIN");

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    eprintln!("=== RECOVERY MNEMONIC: {mnemonic}");

    // Create wallet via onboarding (login)
    let mut wallet = make_wallet(&mnemonic, &config, "Test").await;
    let starknet_addr = wallet.active_account().unwrap().starknet_address;
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    // Send WBTC to the fresh account
    eprintln!("  Sending 1000 sats from faucet...");
    faucet_strk(&config, &starknet_addr, TOKEN_UNIT).await;

    // First refresh → should trigger deploy (not fund yet)
    eprintln!("  First handle_refresh_balance (should deploy)...");
    wallet.handle_refresh_balance().await.ok();
    if let Some(err) = wallet.last_auto_fund_error() {
        eprintln!("  auto_fund_error after first refresh: {err}");
    }

    // Wait for deploy to confirm
    let rpc = RpcClient::new(&config).expect("RpcClient");
    let class_hash = Felt::from_hex(&config.account_class_hash).unwrap();
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if rpc.is_account_deployed(&starknet_addr, &class_hash).await.unwrap_or(false) {
            eprintln!("  Deployed after {attempt} attempts");
            break;
        }
    }

    // Second refresh → should trigger fund (deploy already done)
    eprintln!("  Second handle_refresh_balance (should fund)...");
    wallet.handle_refresh_balance().await.ok();
    if let Some(err) = wallet.last_auto_fund_error() {
        eprintln!("  auto_fund_error after second refresh: {err}");
    } else {
        eprintln!("  No auto_fund_error — fund may have succeeded");
    }

    // Wait for fund to settle
    for attempt in 1..=36 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet.handle_refresh_balance().await.ok();
        let bal = wallet.active_account().unwrap().balance;
        if bal > 0 {
            eprintln!("  Funded! balance={bal} after {attempt} attempts");
            break;
        }
        if let Some(err) = wallet.last_auto_fund_error() {
            eprintln!("  auto_fund_error at attempt {attempt}: {err}");
        }
    }

    let final_bal = wallet.active_account().unwrap().balance;
    eprintln!("  Final balance: {final_bal}");

    // Cleanup: ragequit back to faucet
    if final_bal > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        wallet.handle_refresh_balance().await.ok();
        match wallet.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => {
                eprintln!("  Cleanup ragequit tx: {tx}");
                wait_for_tx(&config, &tx).await;
            }
            Err(e) => eprintln!("  Cleanup ragequit failed: {e}"),
        }
    }
    eprintln!("=== test_auto_fund_on_login_flow complete");
}

/// Prints the faucet (OUBLI_TEST_MNEMONIC_A) Starknet address for mainnet.
/// WARNING: Creates a WalletCore which triggers auto-fund, sweeping public WBTC into Tongo.
/// Run `test_recover_strk_mainnet` afterwards to ragequit the balance back.
#[tokio::test]
#[ignore = "run manually to print mainnet faucet address"]
async fn test_print_faucet_address_mainnet() {
    let config = NetworkConfig::from_env();
    let mnemonic_a =
        std::env::var("OUBLI_TEST_MNEMONIC_A").expect("set OUBLI_TEST_MNEMONIC_A");
    let storage = Box::new(MockPlatformStorage::new());
    let mut wallet = WalletCore::new(storage, config);
    wallet.handle_onboarding(&mnemonic_a).await.unwrap();
    eprintln!(
        "Mainnet faucet — fund this Starknet address with STRK:\n  {:#066x}",
        wallet.active_account().unwrap().starknet_address
    );
}

/// Recovery test: ragequit funds from previous failed runs back to faucet.
/// Set OUBLI_RECOVER_MNEMONICS as a semicolon-separated list of mnemonics.
///
/// NOTE: This creates WalletCore instances which trigger auto-fund. If used with
/// the faucet mnemonic, it will sweep public WBTC → Tongo, then ragequit back.
/// This is safe but costs gas for the round-trip.
#[tokio::test]
#[serial]
#[ignore = "run manually with OUBLI_RECOVER_MNEMONICS env var"]
async fn test_recover_strk_mainnet() {
    let config = NetworkConfig::from_env();
    assert_eq!(config.chain_id, "SN_MAIN", "this test must run against mainnet");

    let mnemonics_str = std::env::var("OUBLI_RECOVER_MNEMONICS")
        .expect("set OUBLI_RECOVER_MNEMONICS='mnemonic1;mnemonic2;...'");
    let mnemonics: Vec<&str> = mnemonics_str.split(';').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);
    eprintln!("Faucet: {faucet_hex}");

    for (i, mnemonic) in mnemonics.iter().enumerate() {
        let label = format!("R{i}");
        let mut wallet = make_wallet(mnemonic, &config, &label).await;
        wallet.handle_refresh_balance().await.ok();

        let acct = wallet.active_account().unwrap();
        let balance = acct.balance;
        let addr = acct.starknet_address;
        eprintln!("  {label}: balance={balance}, addr={:#066x}", addr);

        if balance > 0 {
            eprintln!("  {label}: ragequit → faucet");
            match wallet.handle_ragequit_op(&faucet_hex).await {
                Ok(tx) => {
                    eprintln!("  {label}: ragequit tx: {tx}");
                    wait_for_tx(&config, &tx).await;
                }
                Err(e) => eprintln!("  {label}: ragequit failed: {e}"),
            }
        }
    }
    eprintln!("Recovery complete");
}

// ── Lightning payment test ────────────────────────────────────

/// Resolve a Lightning Address (user@domain) to a fresh BOLT11 invoice via LNURL-pay.
///
/// Protocol: <https://github.com/lnurl/luds/blob/luds/16.md>
/// 1. GET https://<domain>/.well-known/lnurlp/<user> → { callback, minSendable, maxSendable }
/// 2. GET <callback>?amount=<millisats> → { pr: "<bolt11>" }
async fn resolve_ln_address(address: &str, amount_msats: u64) -> String {
    let parts: Vec<&str> = address.split('@').collect();
    assert_eq!(parts.len(), 2, "Lightning Address must be user@domain");
    let (user, domain) = (parts[0], parts[1]);

    let client = reqwest::Client::new();

    // Step 1: Get LNURL-pay metadata
    let meta_url = format!("https://{}/.well-known/lnurlp/{}", domain, user);
    eprintln!("  LNURL-pay: GET {meta_url}");
    let meta_resp: serde_json::Value = client
        .get(&meta_url)
        .send()
        .await
        .expect("LNURL-pay metadata request failed")
        .json()
        .await
        .expect("LNURL-pay metadata parse failed");

    let callback = meta_resp["callback"]
        .as_str()
        .expect("missing callback in LNURL-pay response");
    let min_sendable = meta_resp["minSendable"].as_u64().unwrap_or(1000);
    let max_sendable = meta_resp["maxSendable"].as_u64().unwrap_or(u64::MAX);
    eprintln!(
        "  LNURL-pay: callback={}, min={}msat, max={}msat",
        callback, min_sendable, max_sendable
    );

    assert!(
        amount_msats >= min_sendable && amount_msats <= max_sendable,
        "amount {amount_msats}msat outside LNURL limits [{min_sendable}, {max_sendable}]"
    );

    // Step 2: Request invoice
    let sep = if callback.contains('?') { "&" } else { "?" };
    let invoice_url = format!("{callback}{sep}amount={amount_msats}");
    eprintln!("  LNURL-pay: GET {invoice_url}");
    let invoice_resp: serde_json::Value = client
        .get(&invoice_url)
        .send()
        .await
        .expect("LNURL-pay invoice request failed")
        .json()
        .await
        .expect("LNURL-pay invoice parse failed");

    let bolt11 = invoice_resp["pr"]
        .as_str()
        .expect("missing 'pr' (BOLT11 invoice) in LNURL-pay response")
        .to_string();
    eprintln!(
        "  LNURL-pay: got invoice ({}... {} chars)",
        &bolt11[..40.min(bolt11.len())],
        bolt11.len()
    );

    bolt11
}

/// Pay a Lightning invoice end-to-end on mainnet via Atomiq WBTC→BTCLN swap.
///
/// Requires:
///   - `OUBLI_TEST_LN_ADDRESS` env var — Lightning Address to generate invoices
///     (e.g. `user@getalby.com`, `user@walletofsatoshi.com`).
///     Use a wallet you control (from your own seed phrase) to verify receipt.
///   - `OUBLI_TEST_LN_AMOUNT_SATS` env var (optional, default 10000) — invoice amount in sats.
///     Must be above Atomiq's minimum swap size and within the LNURL-pay limits.
///
/// Flow:
///   1. Resolve Lightning Address → generate fresh BOLT11 invoice
///   2. Create fresh Oubli wallet, fund from faucet
///   3. Auto-fund (deploy + sweep WBTC into Tongo pool)
///   4. `handle_pay_lightning(bolt11)` — withdraw from Tongo + Atomiq swap + LP pays invoice
///   5. Recover remaining WBTC back to faucet
///
/// WARNING: Uses REAL WBTC! Budget: ~20,000 sats (most recovered at end).
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/mainnet.env && set +a
///   OUBLI_TEST_LN_ADDRESS="you@getalby.com" \
///   cargo test -p oubli-wallet --test mainnet_full_flow test_pay_lightning_mainnet -- --ignored --nocapture
#[tokio::test]
#[serial]
#[ignore = "requires MAINNET env + OUBLI_TEST_LN_ADDRESS — uses REAL WBTC"]
async fn test_pay_lightning_mainnet() {
    let config = NetworkConfig::from_env();
    assert_eq!(config.chain_id, "SN_MAIN", "this test must run against mainnet");

    // ── Read env vars ──────────────────────────────────────────
    let ln_address = std::env::var("OUBLI_TEST_LN_ADDRESS")
        .expect("set OUBLI_TEST_LN_ADDRESS (e.g. user@getalby.com)");
    let amount_sats: u64 = std::env::var("OUBLI_TEST_LN_AMOUNT_SATS")
        .unwrap_or_else(|_| "10000".into())
        .parse()
        .expect("OUBLI_TEST_LN_AMOUNT_SATS must be a number");
    let amount_msats = amount_sats * 1000;

    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);
    eprintln!("=== Lightning payment test (mainnet)");
    eprintln!("  LN Address: {ln_address}");
    eprintln!("  Amount: {amount_sats} sats");
    eprintln!("  Faucet: {faucet_hex}");

    // ── Step 1: Generate fresh BOLT11 invoice ──────────────────
    eprintln!("\n=== Step 1: Resolve Lightning Address → BOLT11 invoice");
    let bolt11 = resolve_ln_address(&ln_address, amount_msats).await;
    eprintln!("  Invoice: {}...", &bolt11[..60.min(bolt11.len())]);

    // ── Step 2: Create and fund wallet ─────────────────────────
    eprintln!("\n=== Step 2: Create wallet + fund from faucet");
    let mnemonic = krusty_kms::generate_mnemonic(12).expect("mnemonic generation");
    eprintln!("  RECOVERY MNEMONIC (save in case of failure): {mnemonic}");
    let mut wallet = make_wallet(&mnemonic, &config, "LN-Payer").await;
    let starknet_addr = wallet.active_account().unwrap().starknet_address;

    // Fund with enough for the swap + fees (~2x the invoice amount).
    // Atomiq fees are typically 3-10%, but we add generous buffer.
    let fund_amount = amount_sats as u128 * 2;
    eprintln!("  Faucet → LN-Payer ({fund_amount} sats)");
    faucet_strk(&config, &starknet_addr, fund_amount).await;

    // ── Step 3: Auto-fund (deploy + sweep into Tongo) ──────────
    eprintln!("\n=== Step 3: Auto-fund (deploy + sweep WBTC → Tongo)");
    let tongo_balance = wait_for_auto_fund(&mut wallet, &config, "LN-Payer").await;
    assert!(tongo_balance > 0, "should have Tongo balance after auto-fund");
    eprintln!("  Tongo balance: {tongo_balance}");

    // ── Step 4: Pay Lightning invoice ──────────────────────────
    eprintln!("\n=== Step 4: handle_pay_lightning (withdraw + Atomiq swap)");
    eprintln!("  This may take several minutes (Tongo withdraw + LP negotiation + escrow)...");

    let pay_result = wallet.handle_pay_lightning(&bolt11).await;
    match &pay_result {
        Ok(swap_id) => {
            eprintln!("  Lightning payment succeeded! swap_id={swap_id}");
        }
        Err(e) => {
            eprintln!("  Lightning payment FAILED: {e}");
        }
    }
    let swap_id = pay_result.expect("handle_pay_lightning should succeed");

    // Verify wallet balance decreased
    wallet.handle_refresh_balance().await.ok();
    let balance_after = wallet.active_account().unwrap().balance;
    eprintln!("  Tongo balance after payment: {balance_after}");
    assert!(
        balance_after < tongo_balance,
        "Tongo balance should decrease after Lightning payment"
    );

    // ── Step 5: Cleanup — recover remaining WBTC to faucet ─────
    eprintln!("\n=== Step 5: Cleanup (recover WBTC to faucet)");

    let remaining = wallet.active_account().unwrap().balance;
    if remaining > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        wallet.handle_refresh_balance().await.ok();
        eprintln!("  Ragequit remaining ({remaining}) → faucet");
        match wallet.handle_ragequit_op(&faucet_hex).await {
            Ok(tx) => {
                eprintln!("  ragequit tx: {tx}");
                wait_for_tx(&config, &tx).await;
            }
            Err(e) => eprintln!("  ragequit failed (non-fatal): {e}"),
        }
    }

    // Return any public WBTC from ragequit
    let rpc = RpcClient::new(&config).expect("RpcClient");
    let token = Felt::from_hex(&config.token_contract).unwrap();
    let public_balance = rpc
        .get_erc20_balance(&token, &starknet_addr)
        .await
        .unwrap_or(0);
    if public_balance > TOKEN_UNIT / 10 {
        let send_amount = public_balance.saturating_sub(TOKEN_UNIT / 100);
        match wallet.send_token(&faucet_addr, send_amount).await {
            Ok(tx) => eprintln!("  send_token cleanup tx: {tx}"),
            Err(e) => eprintln!("  send_token cleanup failed (non-fatal): {e}"),
        }
    }

    eprintln!("\n=== DONE: Lightning payment test complete (swap_id={swap_id})");
    eprintln!("  Check your Lightning wallet ({ln_address}) for the {amount_sats} sat payment!");
}

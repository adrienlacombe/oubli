/// Full-flow integration test against Sepolia.
///
/// Tests all wallet operations using 5 seeds (S1–S5) funded from a proxy faucet (S0).
/// Recovers most STRK back to S0 at the end.
///
/// # Faucet architecture
///
/// The faucet (S0, `OUBLI_TEST_MNEMONIC_A`) distributes STRK to test wallets via
/// paymaster-sponsored ERC-20 transfers. This bypasses WalletCore entirely, so the
/// faucet's public token balance is preserved.
///
/// **Do NOT create a `WalletCore` from the faucet mnemonic** during normal test setup.
/// WalletCore's auto-fund will sweep all public tokens into the Tongo privacy pool,
/// making them unavailable for ERC-20 distribution.
///
/// Run with:
///   set -a && . crates/oubli-wallet/tests/sepolia.env && set +a
///   cargo test -p oubli-wallet --test sepolia_full_flow -- --ignored --nocapture
mod support;

use oubli_store::MockPlatformStorage;
use oubli_wallet::config::NetworkConfig;
use oubli_wallet::core::WalletCore;
use oubli_wallet::rpc::RpcClient;
use serial_test::serial;
use starknet_types_core::felt::Felt;
use support::{
    ensure_faucet_deployed_via_paymaster, faucet_starknet_address, faucet_transfer_via_paymaster,
};

/// 1 STRK = 10^18 wei
const ONE_STRK: u128 = 1_000_000_000_000_000_000;

// ── Helpers ───────────────────────────────────────────────────

/// Send STRK from the faucet (OUBLI_TEST_MNEMONIC_A) to a target address via the paymaster.
async fn faucet_strk(config: &NetworkConfig, to_address: &Felt, amount: u128) {
    let tx_hash = faucet_transfer_via_paymaster(config, to_address, amount).await;
    eprintln!("  faucet_strk tx: {tx_hash}");

    wait_for_tx(config, &tx_hash).await;
}

/// Wait for a transaction to be confirmed on-chain (up to ~60s).
async fn wait_for_tx(config: &NetworkConfig, tx_hash: &str) {
    let rpc = RpcClient::new(config).expect("RpcClient");
    for attempt in 1..=12 {
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
async fn wait_for_auto_fund(wallet: &mut WalletCore, config: &NetworkConfig, label: &str) -> u128 {
    let rpc = RpcClient::new(config).expect("RpcClient");
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let starknet_addr = wallet.active_account().unwrap().starknet_address;

    // First refresh → triggers deploy
    wallet.handle_refresh_balance().await.ok();

    // Wait for deploy
    for attempt in 1..=24 {
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
    for attempt in 1..=24 {
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
    for _attempt in 1..=24 {
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

/// Deploy faucet account via paymaster if not already deployed.
async fn ensure_faucet_deployed(config: &NetworkConfig) {
    let rpc = RpcClient::new(config).expect("RpcClient");
    let faucet_addr = faucet_starknet_address(config);
    let class_hash =
        Felt::from_hex(&config.account_class_hash).expect("invalid account class hash");
    let deployed = rpc
        .is_account_deployed(&faucet_addr, &class_hash)
        .await
        .unwrap_or(false);

    if !deployed {
        eprintln!("=== Deploying faucet account via paymaster");
        if let Some(tx_hash) = ensure_faucet_deployed_via_paymaster(config).await {
            eprintln!("  Faucet deploy tx: {tx_hash}");
            wait_for_tx(config, &tx_hash).await;
            eprintln!("  Faucet deployed");
        }
    } else {
        eprintln!("  Faucet already deployed");
    }
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
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_full_flow_sepolia() {
    let config = NetworkConfig::from_env();
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);
    eprintln!("=== Faucet (S0): {faucet_hex}");

    // ── Phase 0: Deploy faucet if needed ─────────────────────
    ensure_faucet_deployed(&config).await;

    // ── Generate seeds ───────────────────────────────────────
    let m1 = krusty_kms::generate_mnemonic(12).unwrap();
    let m2 = krusty_kms::generate_mnemonic(12).unwrap();
    let m3 = krusty_kms::generate_mnemonic(12).unwrap();
    let m4 = krusty_kms::generate_mnemonic(12).unwrap();
    let m5 = krusty_kms::generate_mnemonic(12).unwrap();

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

    // ── Phase 1: Distribute STRK from faucet ─────────────────
    // Distribute sequentially to avoid faucet nonce conflicts.
    // Use small amounts — Tongo operations work with tiny satoshi units (e.g. 10 sats).
    eprintln!("\n=== Phase 1: Distribute STRK from faucet");
    eprintln!("  S0 → S1 (5 STRK)");
    faucet_strk(&config, &s1_addr, 5 * ONE_STRK).await;
    eprintln!("  S0 → S2 (1 STRK)");
    faucet_strk(&config, &s2_addr, ONE_STRK).await;
    eprintln!("  S0 → S3 (2 STRK)");
    faucet_strk(&config, &s3_addr, 2 * ONE_STRK).await;
    eprintln!("  S0 → S4 (2 STRK)");
    faucet_strk(&config, &s4_addr, 2 * ONE_STRK).await;
    eprintln!("  S0 → S5 (1 STRK)");
    faucet_strk(&config, &s5_addr, ONE_STRK).await;

    // ── Phase 2: Auto-fund (deploy + sweep STRK → Tongo) ────
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
    // Send more STRK to S1, then explicitly fund (not via auto-fund).
    // Allow state to settle after all the auto-fund operations in Phase 2.
    eprintln!("\n=== Phase 3A: Explicit fund (S1)");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    faucet_strk(&config, &s1_addr, ONE_STRK).await;

    // Do NOT call handle_refresh_balance here — it would trigger auto-fund
    // which sweeps the STRK before the explicit fund can use it.
    // handle_fund uses sync_balance_for_proof() internally (no auto-fund).
    let fund_tx = retry_paymaster_op(&mut s1, "S1", 3, "fund", "10", "").await;
    eprintln!("  S1 fund tx: {fund_tx}");
    wait_for_tx(&config, &fund_tx).await;

    // Refresh triggers auto-fund of remaining STRK. Wait for it to settle
    // so the on-chain cipher balance is consistent before transfers.
    s1.handle_refresh_balance().await.unwrap();
    for att in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s1.handle_refresh_balance().await.ok();
        let acct = s1.active_account().unwrap();
        // Auto-fund is done when balance stabilizes and no auto-fund error
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

    // Allow state to settle after auto-fund before starting transfers
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    s1.handle_refresh_balance().await.ok();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // S1 → S2 (transfer)
    let s2_pk = s2.active_account().unwrap().owner_public_key_hex.clone();
    let s5_pk = s5.active_account().unwrap().owner_public_key_hex.clone();

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
                                        // Sync S1 so its cipher balance is up-to-date for later operations
    s1.handle_refresh_balance().await.ok();

    // ── Phase 3C: Rollovers ──────────────────────────────────
    eprintln!("\n=== Phase 3C: Rollovers");

    // S2 rollover (received from S1) — auto-rollover handles it via handle_refresh_balance
    eprintln!("  Waiting for S2 rollover (auto)...");
    let s2_bal_after_rollover = wait_for_rollover(&mut s2, "S2").await;
    assert!(
        s2_bal_after_rollover > s2_bal,
        "S2 balance should increase after rollover"
    );

    // S5 rollover (received from S1 + S3) — auto-rollover handles it
    eprintln!("  Waiting for S5 rollover (auto)...");
    let s5_bal_after_rollover = wait_for_rollover(&mut s5, "S5").await;
    assert!(
        s5_bal_after_rollover > s5_bal,
        "S5 balance should increase after rollover"
    );

    // ── Phase 3D: Transfer rolled-over funds (confirm spendable) ─
    eprintln!("\n=== Phase 3D: Transfer rolled-over funds (S2 → S1)");
    let s1_pk = s1.active_account().unwrap().owner_public_key_hex.clone();
    let tx = retry_paymaster_op(&mut s2, "S2→S1", 3, "transfer", "10", &s1_pk).await;
    eprintln!("  S2→S1 tx: {tx}");

    // S1 rollover — auto-rollover picks up pending automatically
    let s1_bal_before = s1.active_account().unwrap().balance;
    eprintln!("  Waiting for S1 balance to increase (auto-rollover)...");
    wait_for_tx(&config, &tx).await;
    for attempt in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s1.handle_refresh_balance().await.ok();
        let bal = s1.active_account().unwrap().balance;
        if bal > s1_bal_before {
            eprintln!("  S1: balance increased {s1_bal_before} → {bal} after {attempt} attempts");
            break;
        }
        assert!(
            attempt < 24,
            "S1 balance never increased after S2→S1 transfer"
        );
    }

    // ── Phase 3E: Withdraw ───────────────────────────────────
    eprintln!("\n=== Phase 3E: Withdraw");

    // S3 → faucet (withdraw from Tongo pool to S0 starknet address)
    s3.handle_refresh_balance().await.ok();
    let s3_bal = s3.active_account().unwrap().balance;
    eprintln!("  S3 balance before withdraw: {s3_bal}");

    eprintln!("  S3 withdraw → faucet");
    let tx = retry_paymaster_op(&mut s3, "S3", 3, "withdraw", "10", &faucet_hex).await;
    eprintln!("  S3 withdraw tx: {tx}");
    wait_for_tx(&config, &tx).await;

    s3.handle_refresh_balance().await.ok();
    let s3_bal_after = s3.active_account().unwrap().balance;
    eprintln!("  S3 balance after withdraw: {s3_bal_after}");
    assert!(
        s3_bal_after < s3_bal,
        "S3 balance should decrease after withdraw"
    );

    // ── Phase 3F: Ragequit ───────────────────────────────────
    eprintln!("\n=== Phase 3F: Ragequit");

    // Let state settle after previous withdraw before attempting ragequit
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // S4 ragequit → faucet (not self, to avoid auto-fund re-sweep)
    s4.handle_refresh_balance().await.ok();
    let s4_bal = s4.active_account().unwrap().balance;
    eprintln!("  S4 balance before ragequit: {s4_bal}");

    eprintln!("  S4 ragequit → faucet");
    let tx = retry_paymaster_op(&mut s4, "S4", 3, "ragequit", "", &faucet_hex).await;
    eprintln!("  S4 ragequit tx: {tx}");
    wait_for_tx(&config, &tx).await;

    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s4.handle_refresh_balance().await.ok();
        let acct = s4.active_account().unwrap();
        if acct.balance == 0 && acct.pending == 0 {
            eprintln!(
                "  S4 Tongo balance=0, pending=0 — ragequit confirmed after {attempt} attempts"
            );
            break;
        }
        assert!(attempt < 12, "S4 should be 0 after ragequit");
    }

    // ── Phase 4: Edge cases ──────────────────────────────────
    eprintln!("\n=== Phase 4: Edge cases");

    // 4.1: S5 ragequit (after rollover of multi-sender transfers)
    // Ragequit to faucet (not self) — otherwise auto-fund would re-sweep the tokens
    eprintln!("  4.1: S5 ragequit → faucet (after multi-sender rollover)");
    s5.handle_refresh_balance().await.ok();
    let tx = retry_paymaster_op(&mut s5, "S5", 3, "ragequit", "", &faucet_hex).await;
    eprintln!("  S5 ragequit tx: {tx}");
    wait_for_tx(&config, &tx).await;

    // Poll until balance reflects the ragequit (latest block may lag)
    for attempt in 1..=12 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s5.handle_refresh_balance().await.ok();
        if s5.active_account().unwrap().balance == 0 {
            eprintln!("  S5 ragequit confirmed after {attempt} attempts");
            break;
        }
        assert!(attempt < 12, "S5 should be 0 after ragequit");
    }

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
    faucet_strk(&config, &s4_addr, ONE_STRK).await;
    // Wait for auto-fund to fully settle (deploy already done, just fund)
    let mut s4_settled = false;
    for att in 1..=24 {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        s4.handle_refresh_balance().await.ok();
        let bal = s4.active_account().unwrap().balance;
        let no_err = s4.last_auto_fund_error().is_none();
        if bal > 0 && no_err {
            // One more refresh to ensure on-chain state is synced
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            s4.handle_refresh_balance().await.ok();
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
    // Ragequit to faucet to avoid auto-fund re-sweep cycle
    eprintln!("  4.4: S4 second ragequit → faucet");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    s4.handle_refresh_balance().await.ok();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let tx = retry_paymaster_op(&mut s4, "S4-2nd", 3, "ragequit", "", &faucet_hex).await;
    eprintln!("  S4 second ragequit tx: {tx}");
    wait_for_tx(&config, &tx).await;
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    s4.handle_refresh_balance().await.ok();
    assert_eq!(
        s4.active_account().unwrap().balance,
        0,
        "S4 balance should be 0 after second ragequit"
    );
    eprintln!("  S4 second ragequit confirmed");

    // ── Phase 5: Activity verification ───────────────────────
    eprintln!("\n=== Phase 5: Activity verification");

    let s1_activity = s1.get_activity().await.expect("S1 get_activity failed");
    eprintln!("  S1 activity: {} events", s1_activity.len());
    for (i, ev) in s1_activity.iter().enumerate() {
        eprintln!(
            "    [{i}] type={} amount={:?} block={}",
            ev.event_type, ev.amount_sats, ev.block_number
        );
    }
    assert!(!s1_activity.is_empty(), "S1 should have activity events");
    // S1 had: auto-fund, explicit fund, 2 transfers out, 1 transfer in, 1 rollover
    let fund_events: Vec<_> = s1_activity
        .iter()
        .filter(|e| e.event_type == "Fund")
        .collect();
    assert!(!fund_events.is_empty(), "S1 should have Fund events");
    let transfer_out: Vec<_> = s1_activity
        .iter()
        .filter(|e| e.event_type == "TransferOut")
        .collect();
    assert!(
        !transfer_out.is_empty(),
        "S1 should have TransferOut events"
    );
    let transfer_in: Vec<_> = s1_activity
        .iter()
        .filter(|e| e.event_type == "TransferIn")
        .collect();
    assert!(!transfer_in.is_empty(), "S1 should have TransferIn events");
    let rollover_events: Vec<_> = s1_activity
        .iter()
        .filter(|e| e.event_type == "Rollover")
        .collect();
    assert!(
        !rollover_events.is_empty(),
        "S1 should have Rollover events"
    );

    let s4_activity = s4.get_activity().await.expect("S4 get_activity failed");
    eprintln!("  S4 activity: {} events", s4_activity.len());
    for (i, ev) in s4_activity.iter().enumerate() {
        eprintln!(
            "    [{i}] type={} amount={:?} block={}",
            ev.event_type, ev.amount_sats, ev.block_number
        );
    }
    let ragequit_events: Vec<_> = s4_activity
        .iter()
        .filter(|e| e.event_type == "Ragequit")
        .collect();
    assert!(
        ragequit_events.len() >= 2,
        "S4 should have at least 2 Ragequit events"
    );

    // ── Phase 6: STRK recovery ───────────────────────────────
    eprintln!("\n=== Phase 6: STRK recovery to faucet");

    // S1: withdraw remaining Tongo balance to faucet
    // Add delays between recovery ragequits to avoid paymaster rate-limiting
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

    // S2: withdraw remaining to faucet
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

    // S3: withdraw remaining to faucet
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

    // S4: already ragequit, but may have public STRK from the ragequit itself.
    // Use send_token to return it.
    s4.handle_refresh_balance().await.ok();
    eprintln!("  S4 send_token (public STRK from ragequit) → faucet");
    let token = Felt::from_hex(&config.token_contract).unwrap();
    let rpc = RpcClient::new(&config).unwrap();
    let s4_public = rpc.get_erc20_balance(&token, &s4_addr).await.unwrap_or(0);
    if s4_public > ONE_STRK / 10 {
        // Keep a small amount for gas, send rest back
        let send_amount = s4_public.saturating_sub(ONE_STRK / 100);
        match s4.send_token(&faucet_addr, send_amount).await {
            Ok(tx) => eprintln!("  S4 send_token tx: {tx}"),
            Err(e) => eprintln!("  S4 send_token failed (non-fatal): {e}"),
        }
    }

    // S5: same as S4
    s5.handle_refresh_balance().await.ok();
    eprintln!("  S5 send_token (public STRK from ragequit) → faucet");
    let s5_public = rpc.get_erc20_balance(&token, &s5_addr).await.unwrap_or(0);
    if s5_public > ONE_STRK / 10 {
        let send_amount = s5_public.saturating_sub(ONE_STRK / 100);
        match s5.send_token(&faucet_addr, send_amount).await {
            Ok(tx) => eprintln!("  S5 send_token tx: {tx}"),
            Err(e) => eprintln!("  S5 send_token failed (non-fatal): {e}"),
        }
    }

    eprintln!("\n=== DONE: Full flow test complete, STRK recovered to faucet");
}

/// Focused test: withdraw operation.
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_withdraw_sepolia() {
    let config = NetworkConfig::from_env();
    ensure_faucet_deployed(&config).await;
    let faucet_addr = faucet_starknet_address(&config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut wallet = make_wallet(&mnemonic, &config, "withdraw-test").await;
    let addr = wallet.active_account().unwrap().starknet_address;

    // Fund via faucet + auto-fund
    faucet_strk(&config, &addr, ONE_STRK).await;
    let balance = wait_for_auto_fund(&mut wallet, &config, "withdraw-test").await;
    assert!(balance > 0);

    // Withdraw a small amount to faucet
    let tx = wallet
        .handle_withdraw_op("10", &faucet_hex)
        .await
        .expect("withdraw failed");
    eprintln!("withdraw tx: {tx}");
    wait_for_tx(&config, &tx).await;

    wallet.handle_refresh_balance().await.unwrap();
    let new_balance = wallet.active_account().unwrap().balance;
    assert!(
        new_balance < balance,
        "balance should decrease after withdraw"
    );

    // Cleanup: ragequit remaining to faucet
    if new_balance > 0 {
        wallet.handle_ragequit_op(&faucet_hex).await.ok();
    }
}

/// Focused test: external withdraw with app fee enabled.
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_withdraw_with_fee_sepolia() {
    let base_config = NetworkConfig::from_env();
    ensure_faucet_deployed(&base_config).await;
    let faucet_addr = faucet_starknet_address(&base_config);
    let faucet_hex = format!("{:#066x}", faucet_addr);

    let collector_mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut collector = make_wallet(&collector_mnemonic, &base_config, "fee-collector").await;
    let collector_pubkey = collector
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();

    let mut fee_config = base_config.clone();
    fee_config.fee_percent = 1.0;
    fee_config.fee_collector_pubkey = Some(collector_pubkey);

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut wallet = make_wallet(&mnemonic, &fee_config, "withdraw-fee-test").await;
    let addr = wallet.active_account().unwrap().starknet_address;

    faucet_strk(&fee_config, &addr, ONE_STRK).await;
    let balance_before = wait_for_auto_fund(&mut wallet, &fee_config, "withdraw-fee-test").await;
    assert!(balance_before > 0);

    let rpc = RpcClient::new(&fee_config).unwrap();
    let token_contract = Felt::from_hex(&fee_config.token_contract).unwrap();
    let rate = rpc.contract().get_rate().await.unwrap() as u128;
    let faucet_public_before = rpc
        .get_erc20_balance(&token_contract, &faucet_addr)
        .await
        .expect("get faucet balance before withdraw");

    let amount_sats = "10";
    let amount_units = oubli_wallet::sats_to_tongo_units(amount_sats).unwrap() as u128;
    let fee_sats = oubli_wallet::calculate_fee_sats(
        amount_sats.parse::<u64>().unwrap(),
        fee_config.fee_percent,
    );
    let fee_units = oubli_wallet::sats_to_tongo_units(&fee_sats.to_string()).unwrap() as u128;

    let tx = retry_paymaster_op(
        &mut wallet,
        "withdraw-fee-test",
        3,
        "withdraw",
        amount_sats,
        &faucet_hex,
    )
    .await;
    eprintln!("withdraw with fee tx: {tx}");
    wait_for_tx(&fee_config, &tx).await;

    wallet.handle_refresh_balance().await.unwrap();
    let sender_after = wallet.active_account().unwrap().balance;
    assert_eq!(
        sender_after,
        balance_before - amount_units - fee_units,
        "sender balance should decrease by withdraw amount plus fee"
    );

    let faucet_public_after = rpc
        .get_erc20_balance(&token_contract, &faucet_addr)
        .await
        .expect("get faucet balance after withdraw");
    assert_eq!(
        faucet_public_after,
        faucet_public_before + (amount_units * rate),
        "public recipient should only receive the withdraw amount"
    );

    let mut collector_balance = 0;
    for attempt in 1..=12 {
        collector.handle_refresh_balance().await.ok();
        collector_balance = collector.active_account().unwrap().balance;
        if collector_balance >= fee_units {
            eprintln!(
                "  fee-collector: credited after {attempt} attempts, balance={collector_balance}"
            );
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    assert_eq!(
        collector_balance, fee_units,
        "collector should receive the fee privately"
    );

    if sender_after > 0 {
        wallet.handle_ragequit_op(&faucet_hex).await.ok();
    }
}

/// Focused test: send_token (ERC-20 transfer via paymaster).
#[tokio::test]
#[serial]
#[ignore = "requires Sepolia env (source sepolia.env)"]
async fn test_send_token_sepolia() {
    let config = NetworkConfig::from_env();
    ensure_faucet_deployed(&config).await;
    let faucet_addr = faucet_starknet_address(&config);

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut wallet = make_wallet(&mnemonic, &config, "send-strk-test").await;
    let addr = wallet.active_account().unwrap().starknet_address;

    // Fund the wallet with STRK and deploy via auto-fund
    faucet_strk(&config, &addr, ONE_STRK).await;
    wait_for_auto_fund(&mut wallet, &config, "send-strk-test").await;

    // Send more STRK (we need public STRK to test send_token)
    faucet_strk(&config, &addr, ONE_STRK).await;

    // send_token: L1 ERC-20 transfer back to faucet
    let tx = wallet
        .send_token(&faucet_addr, ONE_STRK)
        .await
        .expect("send_token failed");
    eprintln!("send_token tx: {tx}");
    wait_for_tx(&config, &tx).await;

    // Verify the faucet received it (check ERC-20 balance)
    let rpc = RpcClient::new(&config).unwrap();
    let token = Felt::from_hex(&config.token_contract).unwrap();
    let wallet_public = rpc
        .get_erc20_balance(&token, &addr)
        .await
        .expect("get_erc20_balance failed");
    eprintln!("wallet public STRK after send_token: {wallet_public}");
    // Should have less than 2 STRK (sent 1 back)
    assert!(
        wallet_public < 2 * ONE_STRK,
        "wallet should have <2 STRK after sending 1 back"
    );

    // Cleanup: ragequit Tongo balance to faucet
    let faucet_hex = format!("{:#066x}", faucet_addr);
    wallet.handle_ragequit_op(&faucet_hex).await.ok();
}

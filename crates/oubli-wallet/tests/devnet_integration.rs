#![cfg(feature = "devnet")]

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use krusty_kms_client::starknet_rust::accounts::{
    Account, AccountFactory, ExecutionEncoding, OpenZeppelinAccountFactory, SingleOwnerAccount,
};
use krusty_kms_client::starknet_rust::core::types::{
    contract::SierraClass, Call, Felt, FlattenedSierraClass,
};
use krusty_kms_client::starknet_rust::providers::jsonrpc::HttpTransport;
use krusty_kms_client::starknet_rust::providers::{JsonRpcClient, Provider};
use krusty_kms_client::starknet_rust::signers::{LocalWallet, SigningKey};

use oubli_store::MockPlatformStorage;
use oubli_wallet::config::NetworkConfig;
use oubli_wallet::core::WalletCore;
use oubli_wallet::networks;
use oubli_wallet::state::WalletState;
use oubli_wallet::submitter::DirectSubmitter;

// ── Constants ─────────────────────────────────────────────────

/// Universal Deployer Contract address on Starknet
const UDC_ADDRESS: Felt = Felt::from_hex_unchecked(
    "0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf",
);

/// UDC deploy selector (devnet UDC entrypoint)
const UDC_DEPLOY_SELECTOR: Felt = Felt::from_hex_unchecked(
    "0x1987cbd17808b9a23693d4de7e246a443cfe37e6e7fbaeabd7d7e6532b07c3d",
);

/// STRK ERC-20 token address on Starknet (same address on devnet, testnet, mainnet)
const STRK_TOKEN_ADDRESS: Felt = Felt::from_hex_unchecked(
    "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d",
);

// ── DevnetFixture ───────────────────────────────────────────

struct DevnetFixture {
    process: Child,
    config: NetworkConfig,
}

#[derive(serde::Deserialize)]
struct JsonRpcResponse<T> {
    result: T,
}

#[derive(serde::Deserialize)]
struct PredeployedAccount {
    address: String,
    private_key: String,
    #[allow(dead_code)]
    public_key: String,
}

impl DevnetFixture {
    async fn setup() -> Self {
        // 1. Spawn starknet-devnet
        let process = Command::new("starknet-devnet")
            .args(["--seed", "42", "--port", "5050"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn starknet-devnet. Is it installed?");

        // Wait for devnet to be ready
        let http = reqwest::Client::new();
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if http
                .get("http://localhost:5050/is_alive")
                .send()
                .await
                .is_ok()
            {
                break;
            }
        }

        // 2. Fetch predeployed accounts (JSON-RPC in devnet v0.7+)
        let resp = http
            .post("http://localhost:5050/rpc")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "devnet_getPredeployedAccounts"
            }))
            .send()
            .await
            .expect("Failed to get predeployed accounts");
        let rpc_resp: JsonRpcResponse<Vec<PredeployedAccount>> =
            resp.json().await.expect("Failed to parse predeployed accounts");
        let accounts = rpc_resp.result;
        let deployer = &accounts[0];

        // 3. Set up starknet-rs provider + account
        let provider = JsonRpcClient::new(HttpTransport::new(
            url::Url::parse("http://localhost:5050/rpc").unwrap(),
        ));

        let deployer_address = Felt::from_hex(&deployer.address).unwrap();
        let deployer_pk = Felt::from_hex(&deployer.private_key).unwrap();
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(deployer_pk));

        let chain_id = provider.chain_id().await.expect("Failed to get chain_id");

        let deployer_account = SingleOwnerAccount::new(
            &provider,
            &signer,
            deployer_address,
            chain_id,
            ExecutionEncoding::New,
        );

        // 4. Declare the Tongo Sierra class
        let class_json_path = format!(
            "{}/devnet/tongo_class.json",
            env!("CARGO_MANIFEST_DIR").replace("/crates/oubli-wallet", "")
        );
        let class_json = std::fs::read_to_string(&class_json_path)
            .expect("Failed to read tongo_class.json");

        // Try parsing as SierraClass (with debug info) first, fall back to FlattenedSierraClass
        let flattened: FlattenedSierraClass =
            if let Ok(sierra) = serde_json::from_str::<SierraClass>(&class_json) {
                sierra.flatten().expect("Failed to flatten Sierra class")
            } else {
                serde_json::from_str(&class_json).expect("Failed to parse FlattenedSierraClass")
            };

        let class_hash = flattened.class_hash();
        eprintln!("Sierra class hash: {class_hash:#066x}");

        // Compiled (CASM) class hash — deterministic for this Sierra artifact.
        // If tongo_class.json changes, update this value from the devnet error message.
        let compiled_class_hash = Felt::from_hex_unchecked(
            "0x14ac3d08acd299becd93b40995e7bbde4944d8895d2718f9dfb8afbaec39243",
        );

        let declare_result = deployer_account
            .declare_v3(Arc::new(flattened), compiled_class_hash)
            .send()
            .await;

        match &declare_result {
            Ok(resp) => eprintln!("Declared class: {:#066x}", resp.class_hash),
            Err(e) => {
                let err_str = format!("{e}");
                if err_str.contains("already declared") || err_str.contains("CLASS_ALREADY_DECLARED") {
                    eprintln!("Class already declared, continuing...");
                } else {
                    panic!("Failed to declare class: {e}");
                }
            }
        }

        // 5. Deploy Tongo contract via UDC
        // Constructor calldata: owner, ERC20 (STRK), rate_low, rate_high, bit_size, auditor_key(Option::None=0)
        let constructor_calldata = vec![
            deployer_address,
            STRK_TOKEN_ADDRESS,
            Felt::ONE,          // rate low
            Felt::ZERO,         // rate high
            Felt::from(32u64),  // bit_size
            Felt::ONE,          // auditor_key = Option::None (variant index 1)
        ];

        let mut udc_calldata = vec![
            class_hash,           // class_hash
            Felt::ZERO,           // salt
            Felt::ZERO,           // unique = false
            Felt::from(constructor_calldata.len() as u64), // calldata_len
        ];
        udc_calldata.extend(constructor_calldata);

        let deploy_result = deployer_account
            .execute_v3(vec![Call {
                to: UDC_ADDRESS,
                selector: UDC_DEPLOY_SELECTOR,
                calldata: udc_calldata,
            }])
            .send()
            .await
            .expect("Failed to deploy Tongo via UDC");

        eprintln!("Deploy tx: {:#066x}", deploy_result.transaction_hash);

        // Wait for deploy tx and extract contract address from events
        tokio::time::sleep(Duration::from_secs(1)).await;
        let receipt = provider
            .get_transaction_receipt(deploy_result.transaction_hash)
            .await
            .expect("Failed to get deploy receipt");

        // Extract the contract address from UDC ContractDeployed event
        let tongo_contract = extract_deployed_address(&receipt)
            .expect("Failed to extract deployed contract address from receipt");

        let tongo_hex = format!("{tongo_contract:#066x}");
        eprintln!("Deployed Tongo at: {tongo_hex}");

        let config = networks::devnet::config(&tongo_hex);

        DevnetFixture { process, config }
    }

    fn make_wallet(&self, _mnemonic: &str, _pin: &str) -> WalletCore {
        let storage = Box::new(MockPlatformStorage::new());
        let submitter = Box::new(DirectSubmitter::new(
            &self.config.rpc_url,
            &self.config.chain_id,
        ));
        WalletCore::new_with_submitter(storage, self.config.clone(), submitter)
    }

    /// Deploy an OZ account contract on devnet for the given wallet.
    /// Must be called after minting tokens to the wallet's address.
    async fn deploy_account(&self, wallet: &WalletCore) {
        let acct = wallet.active_account().unwrap();
        let address_hex = format!("{:#066x}", acct.starknet_address);

        // Mint STRK for v3 deploy gas fees
        self.mint_strk(&address_hex, "1000000000000000000").await;

        let sk_felt = Felt::from_bytes_be(&acct.starknet_private_key.to_bytes_be());
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(sk_felt));

        let provider = JsonRpcClient::new(HttpTransport::new(
            url::Url::parse("http://localhost:5050/rpc").unwrap(),
        ));

        let chain_id = provider.chain_id().await.expect("Failed to get chain_id");
        let class_hash = Felt::from_hex(&self.config.account_class_hash).unwrap();

        let factory = OpenZeppelinAccountFactory::new(class_hash, chain_id, &signer, &provider)
            .await
            .expect("Failed to create account factory");

        // salt=0, same as krusty_kms::derive_oz_account_address with salt=None
        let deployment = factory.deploy_v3(Felt::ZERO);
        let result = deployment.send().await.expect("Failed to deploy account");

        eprintln!(
            "Deployed account {:#066x} tx: {:#066x}",
            result.contract_address, result.transaction_hash
        );
    }

    /// Fund a Starknet address with STRK from the devnet faucet.
    /// STRK is used for both v3 gas fees and as the Tongo ERC20 token.
    async fn mint_strk(&self, address: &str, amount_fri: &str) {
        self.mint_token(address, amount_fri, "FRI").await;
    }

    /// Mint tokens via devnet JSON-RPC. Unit "WEI" for ETH, "FRI" for STRK.
    async fn mint_token(&self, address: &str, amount: &str, unit: &str) {
        let client = reqwest::Client::new();
        let resp = client
            .post("http://localhost:5050/rpc")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "devnet_mint",
                "params": {
                    "address": address,
                    "amount": amount.parse::<u128>().unwrap(),
                    "unit": unit
                }
            }))
            .send()
            .await
            .expect("Failed to mint token");
        assert!(
            resp.status().is_success(),
            "Mint failed: {}",
            resp.text().await.unwrap_or_default()
        );
    }
}

impl Drop for DevnetFixture {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Extract the deployed contract address from a UDC deploy receipt.
fn extract_deployed_address(
    receipt: &krusty_kms_client::starknet_rust::core::types::TransactionReceiptWithBlockInfo,
) -> Option<Felt> {
    use krusty_kms_client::starknet_rust::core::types::{
        ExecutionResult, TransactionReceipt,
    };

    // Check execution succeeded
    let exec_result = match &receipt.receipt {
        TransactionReceipt::Invoke(r) => &r.execution_result,
        TransactionReceipt::Declare(r) => &r.execution_result,
        _ => return None,
    };
    match exec_result {
        ExecutionResult::Succeeded => {}
        ExecutionResult::Reverted { reason } => {
            panic!("Deploy tx reverted: {reason}");
        }
    }

    // The UDC emits a ContractDeployed event with data[0] = contract_address
    let events = match &receipt.receipt {
        TransactionReceipt::Invoke(r) => &r.events,
        _ => return None,
    };
    for event in events {
        if event.from_address == UDC_ADDRESS && !event.data.is_empty() {
            return Some(event.data[0]);
        }
    }
    None
}

// ── Tests ───────────────────────────────────────────────────

/// Diagnostic test: bypass wallet layer entirely and call krusty_kms_sdk + krusty_kms_client directly.
/// This isolates whether the "Proof Of Ownership failed" is in the wallet orchestration
/// or in the proof generation / contract interaction.
#[tokio::test]
async fn test_devnet_direct_fund() {
    use starknet_types_core::felt::Felt as CoreFelt;

    let fixture = DevnetFixture::setup().await;

    // 1. Create a TongoAccount directly
    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let tongo_addr = CoreFelt::from_hex(&fixture.config.tongo_contract).unwrap();
    let tongo_account =
        krusty_kms_sdk::TongoAccount::from_mnemonic(&mnemonic, 0, 0, tongo_addr, None).unwrap();

    // 2. Derive the Starknet account (same logic as wallet core.rs:597)
    let starknet_sk = krusty_kms::derive_private_key_with_coin_type(&mnemonic, 0, 0, 9004, None).unwrap();
    let sk_felt = Felt::from_bytes_be(&starknet_sk.to_bytes_be());
    let signer = LocalWallet::from(SigningKey::from_secret_scalar(sk_felt));

    let provider = JsonRpcClient::new(HttpTransport::new(
        url::Url::parse("http://localhost:5050/rpc").unwrap(),
    ));
    let chain_id = provider.chain_id().await.unwrap();

    let class_hash = Felt::from_hex(&fixture.config.account_class_hash).unwrap();
    let factory = OpenZeppelinAccountFactory::new(class_hash, chain_id, &signer, &provider)
        .await
        .unwrap();

    let deployment = factory.deploy_v3(Felt::ZERO);
    let account_address = deployment.address();
    let address_hex = format!("{account_address:#066x}");
    eprintln!("Account address: {address_hex}");

    // Mint STRK and deploy account
    fixture.mint_strk(&address_hex, "10000000000000000000").await; // 10 STRK
    let deploy_result = deployment.send().await.unwrap();
    eprintln!("Deployed account tx: {:#066x}", deploy_result.transaction_hash);
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 3. Build fund proof directly via krusty_kms_sdk
    // Use chain_id from RPC (already fetched above) so the proof matches what the contract's get_chain_id() sees
    let chain_id_core = CoreFelt::from_bytes_be(&chain_id.to_bytes_be());

    // For a new account, cipher_balance is identity (zero ciphertexts)
    let identity = starknet_types_core::curve::ProjectivePoint::identity();
    let cipher = krusty_kms_common::ElGamalCiphertext {
        l: identity.clone(),
        r: identity.clone(),
    };

    let amount: u128 = 10; // smallest meaningful amount

    let params = krusty_kms_sdk::operations::FundParams {
        amount,
        nonce: CoreFelt::ZERO,
        chain_id: chain_id_core,
        tongo_address: tongo_addr,
        auditor_pub_key: None,
        current_balance: cipher,
    };

    // Print all prefix inputs for debugging
    let y = &tongo_account.keypair.public_key;
    let y_affine = y.to_affine().unwrap();
    eprintln!("=== Fund Proof Prefix Inputs ===");
    eprintln!("  chain_id:       {:#066x}", chain_id_core);
    eprintln!("  tongo_address:  {:#066x}", tongo_addr);
    eprintln!("  FUND_STRING:    {:#066x}", CoreFelt::from_hex("0x66756e64").unwrap());
    eprintln!("  y.x:            {:#066x}", y_affine.x());
    eprintln!("  y.y:            {:#066x}", y_affine.y());
    eprintln!("  amount:         {:#066x}", CoreFelt::from(amount));
    eprintln!("  nonce:          {:#066x}", CoreFelt::ZERO);

    let proof = krusty_kms_sdk::fund(&tongo_account, params).unwrap();

    // 4. Locally verify the proof
    let prefix_inputs = vec![
        chain_id_core,
        tongo_addr,
        CoreFelt::from_hex("0x66756e64").unwrap(),
        y_affine.x(),
        y_affine.y(),
        CoreFelt::from(amount),
        CoreFelt::ZERO,
    ];
    let prefix = krusty_kms_crypto::hash::poseidon_hash_many(&prefix_inputs);
    eprintln!("  prefix hash:    {:#066x}", prefix);

    let local_valid = krusty_kms_crypto::poe::ProofOfExponentiation::verify(y, &proof.proof, &prefix).unwrap();
    eprintln!("  local verify:   {}", local_valid);
    assert!(local_valid, "Local PoE verification should pass!");

    // 5. Build the calls via krusty_kms_client
    let erc20_addr = CoreFelt::from_hex(&fixture.config.token_contract).unwrap();

    // Get rate from contract
    let rpc = oubli_wallet::RpcClient::new(&fixture.config).unwrap();
    let rate = rpc.contract().get_rate().await.unwrap();
    eprintln!("  contract rate:  {}", rate);

    let (hint_ct, hint_nonce) = ([0u8; 64], [0u8; 24]);
    let (approve_call, fund_call) =
        krusty_kms_client::build_fund_calls(tongo_addr, erc20_addr, rate, &proof, &hint_ct, &hint_nonce)
            .unwrap();

    eprintln!("  approve calldata len: {}", approve_call.calldata.len());
    eprintln!("  fund calldata len:    {}", fund_call.calldata.len());
    eprintln!("  fund calldata:");
    for (i, f) in fund_call.calldata.iter().enumerate() {
        eprintln!("    [{i:2}] {f:#066x}");
    }

    // 6. Submit via SingleOwnerAccount
    let soa = SingleOwnerAccount::new(
        &provider,
        &signer,
        account_address,
        chain_id,
        ExecutionEncoding::New,
    );

    // Try fund-only first (no approve) to isolate proof verification
    let fund_call_clone = Call {
        to: fund_call.to,
        selector: fund_call.selector,
        calldata: fund_call.calldata.clone(),
    };

    let result_fund_only = soa
        .execute_v3(vec![fund_call])
        .send()
        .await;

    match &result_fund_only {
        Ok(resp) => eprintln!("Fund-only tx: {:#066x}", resp.transaction_hash),
        Err(e) => {
            let err_str = format!("{e}");
            if err_str.contains("Proof Of Ownership") {
                eprintln!("Fund-only: PROOF FAILED (same error → issue is in proof/contract)");
            } else {
                eprintln!("Fund-only: DIFFERENT ERROR → {e}");
            }
        }
    }

    // Now try full multicall (with new nonce, so need fresh account)
    // Skip this if fund-only succeeded
    if result_fund_only.is_ok() {
        eprintln!("Fund-only succeeded! The approve+fund multicall was the issue.");
    } else {
        // Try the multicall too
        let result = soa
            .execute_v3(vec![approve_call, fund_call_clone])
            .send()
            .await;

        match &result {
            Ok(resp) => eprintln!("Fund multicall tx: {:#066x}", resp.transaction_hash),
            Err(e) => eprintln!("Fund multicall FAILED: {e}"),
        }
    }

    assert!(result_fund_only.is_ok(), "Direct fund failed: {:?}", result_fund_only.err());
}

#[tokio::test]
async fn test_devnet_rpc_reads() {
    let fixture = DevnetFixture::setup().await;

    // Create an RPC client and query contract state
    let rpc = oubli_wallet::RpcClient::new(&fixture.config)
        .expect("Failed to create RPC client");

    let rate = rpc
        .contract()
        .get_rate()
        .await
        .expect("Failed to get rate");
    assert_eq!(rate, 1, "Rate should be 1 (set in constructor)");

    let bit_size = rpc
        .contract()
        .get_bit_size()
        .await
        .expect("Failed to get bit_size");
    assert_eq!(bit_size, 32, "Bit size should be 32 (set in constructor)");
}

#[tokio::test]
async fn test_devnet_fund_and_check_balance() {
    let fixture = DevnetFixture::setup().await;

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut wallet = fixture.make_wallet(&mnemonic, "839201");

    // Onboard
    let result = wallet.handle_onboarding(&mnemonic, "839201").await;
    assert!(result.is_ok(), "Onboarding failed: {:?}", result.err());
    assert!(matches!(wallet.state(), WalletState::Ready { .. }));

    let acct = wallet.active_account().unwrap();
    let address_hex = format!("{:#066x}", acct.starknet_address);

    // Mint STRK (used for both gas and Tongo token)
    fixture.mint_strk(&address_hex, "1000000000000000000").await; // 1 STRK
    fixture.deploy_account(&wallet).await;

    // Fund
    let result = wallet.handle_fund("10").await;
    assert!(result.is_ok(), "Fund failed: {:?}", result.err());

    // Refresh balance from chain
    let result = wallet.handle_refresh_balance().await;
    assert!(result.is_ok(), "Refresh failed: {:?}", result.err());

    let acct = wallet.active_account().unwrap();
    assert!(acct.balance > 0, "Balance should be > 0 after fund");
}

#[tokio::test]
async fn test_devnet_fund_then_rollover() {
    let fixture = DevnetFixture::setup().await;

    let mnemonic_a = krusty_kms::generate_mnemonic(12).unwrap();
    let mnemonic_b = krusty_kms::generate_mnemonic(12).unwrap();

    // Setup wallet A
    let mut wallet_a = fixture.make_wallet(&mnemonic_a, "839201");
    wallet_a.handle_onboarding(&mnemonic_a, "839201").await.unwrap();
    let addr_a = format!("{:#066x}", wallet_a.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr_a, "1000000000000000000").await;
    fixture.deploy_account(&wallet_a).await;

    // Fund wallet A
    wallet_a.handle_fund("10").await
        .expect("Fund A failed");

    // Setup wallet B
    let mut wallet_b = fixture.make_wallet(&mnemonic_b, "839201");
    wallet_b.handle_onboarding(&mnemonic_b, "839201").await.unwrap();
    let addr_b = format!("{:#066x}", wallet_b.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr_b, "1000000000000000000").await;
    fixture.deploy_account(&wallet_b).await;

    // Transfer A → B
    let recipient_pk = wallet_b
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();
    wallet_a
        .handle_transfer_op("10", &recipient_pk)
        .await
        .expect("Transfer A→B failed");

    // Refresh B to see pending
    wallet_b.handle_refresh_balance().await.unwrap();
    let acct_b = wallet_b.active_account().unwrap();
    assert!(acct_b.pending > 0, "B should have pending balance after transfer");

    // Rollover B
    wallet_b.handle_rollover_op().await.expect("Rollover B failed");

    // Refresh B balance
    wallet_b.handle_refresh_balance().await.unwrap();
    let acct_b = wallet_b.active_account().unwrap();
    assert!(acct_b.balance > 0, "B should have balance after rollover");
    assert_eq!(acct_b.pending, 0, "B pending should be 0 after rollover");
}

#[tokio::test]
async fn test_devnet_fund_and_withdraw() {
    let fixture = DevnetFixture::setup().await;

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut wallet = fixture.make_wallet(&mnemonic, "839201");
    wallet.handle_onboarding(&mnemonic, "839201").await.unwrap();

    let addr = format!("{:#066x}", wallet.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr, "1000000000000000000").await;
    fixture.deploy_account(&wallet).await;

    // Fund
    wallet.handle_fund("10").await.expect("Fund failed");

    // Withdraw back to same address
    wallet
        .handle_withdraw_op("10", &addr)
        .await
        .expect("Withdraw failed");

    // Balance should be 0 after withdraw
    wallet.handle_refresh_balance().await.unwrap();
    let acct = wallet.active_account().unwrap();
    assert_eq!(acct.balance, 0, "Balance should be 0 after full withdraw");
}

#[tokio::test]
async fn test_devnet_fund_and_ragequit() {
    let fixture = DevnetFixture::setup().await;

    let mnemonic = krusty_kms::generate_mnemonic(12).unwrap();
    let mut wallet = fixture.make_wallet(&mnemonic, "839201");
    wallet.handle_onboarding(&mnemonic, "839201").await.unwrap();

    let addr = format!("{:#066x}", wallet.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr, "1000000000000000000").await;
    fixture.deploy_account(&wallet).await;

    // Fund
    wallet.handle_fund("10").await.expect("Fund failed");

    // Ragequit
    wallet
        .handle_ragequit_op(&addr)
        .await
        .expect("Ragequit failed");

    // Balance should be 0
    wallet.handle_refresh_balance().await.unwrap();
    let acct = wallet.active_account().unwrap();
    assert_eq!(acct.balance, 0, "Balance should be 0 after ragequit");
    assert_eq!(acct.pending, 0, "Pending should be 0 after ragequit");
}

#[tokio::test]
async fn test_devnet_transfer_between_accounts() {
    let fixture = DevnetFixture::setup().await;

    let mnemonic_a = krusty_kms::generate_mnemonic(12).unwrap();
    let mnemonic_b = krusty_kms::generate_mnemonic(12).unwrap();

    // Setup and fund wallet A
    let mut wallet_a = fixture.make_wallet(&mnemonic_a, "839201");
    wallet_a.handle_onboarding(&mnemonic_a, "839201").await.unwrap();
    let addr_a = format!("{:#066x}", wallet_a.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr_a, "1000000000000000000").await;
    fixture.deploy_account(&wallet_a).await;
    wallet_a.handle_fund("100").await.expect("Fund A failed");

    // Setup wallet B (needs ETH for gas in direct submission mode)
    let mut wallet_b = fixture.make_wallet(&mnemonic_b, "839201");
    wallet_b.handle_onboarding(&mnemonic_b, "839201").await.unwrap();
    let addr_b = format!("{:#066x}", wallet_b.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr_b, "1000000000000000000").await;
    fixture.deploy_account(&wallet_b).await;

    // Transfer A → B
    let recipient_pk = wallet_b
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();
    wallet_a
        .handle_transfer_op("50", &recipient_pk)
        .await
        .expect("Transfer failed");

    // B should have pending
    wallet_b.handle_refresh_balance().await.unwrap();
    assert!(
        wallet_b.active_account().unwrap().pending > 0,
        "B should have pending after transfer"
    );

    // B rollovers
    wallet_b.handle_rollover_op().await.expect("Rollover B failed");
    wallet_b.handle_refresh_balance().await.unwrap();
    assert!(
        wallet_b.active_account().unwrap().balance > 0,
        "B should have balance after rollover"
    );
}

#[tokio::test]
async fn test_devnet_full_lifecycle() {
    let fixture = DevnetFixture::setup().await;

    let mnemonic_a = krusty_kms::generate_mnemonic(12).unwrap();
    let mnemonic_b = krusty_kms::generate_mnemonic(12).unwrap();

    // 1. Onboard wallet A
    let mut wallet_a = fixture.make_wallet(&mnemonic_a, "839201");
    wallet_a
        .handle_onboarding(&mnemonic_a, "839201")
        .await
        .expect("Onboard A failed");
    assert!(matches!(wallet_a.state(), WalletState::Ready { .. }));

    let addr_a = format!("{:#066x}", wallet_a.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr_a, "1000000000000000000").await;
    fixture.deploy_account(&wallet_a).await;

    // 2. Fund A
    wallet_a
        .handle_fund("100")
        .await
        .expect("Fund A failed");
    wallet_a.handle_refresh_balance().await.unwrap();
    let balance_a = wallet_a.active_account().unwrap().balance;
    assert!(balance_a > 0, "A should have balance after fund");

    // 3. Onboard wallet B
    let mut wallet_b = fixture.make_wallet(&mnemonic_b, "839201");
    wallet_b
        .handle_onboarding(&mnemonic_b, "839201")
        .await
        .expect("Onboard B failed");
    let addr_b = format!("{:#066x}", wallet_b.active_account().unwrap().starknet_address);
    fixture.mint_strk(&addr_b, "1000000000000000000").await;
    fixture.deploy_account(&wallet_b).await;

    // 4. Transfer A → B
    let pk_b = wallet_b
        .active_account()
        .unwrap()
        .owner_public_key_hex
        .clone();
    wallet_a
        .handle_transfer_op("50", &pk_b)
        .await
        .expect("Transfer A→B failed");

    // 5. Rollover B
    wallet_b.handle_refresh_balance().await.unwrap();
    assert!(wallet_b.active_account().unwrap().pending > 0);
    wallet_b
        .handle_rollover_op()
        .await
        .expect("Rollover B failed");

    // 6. Withdraw B
    wallet_b.handle_refresh_balance().await.unwrap();
    let balance_b = wallet_b.active_account().unwrap().balance;
    assert!(balance_b > 0, "B should have balance before withdraw");

    let amount_sats = oubli_wallet::tongo_units_to_sats(balance_b as u64);
    wallet_b
        .handle_withdraw_op(&amount_sats, &addr_b)
        .await
        .expect("Withdraw B failed");

    wallet_b.handle_refresh_balance().await.unwrap();
    assert_eq!(
        wallet_b.active_account().unwrap().balance,
        0,
        "B balance should be 0 after withdraw"
    );
}

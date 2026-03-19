use crate::config::NetworkConfig;
use crate::core::ActiveAccount;
use crate::error::WalletError;
use crate::paymaster::PaymasterClient;
use crate::rpc::RpcClient;
use crate::signing;
use starknet_types_core::felt::Felt;

/// Trait for submitting transactions on behalf of an account.
#[async_trait::async_trait]
pub trait TransactionSubmitter: Send + Sync {
    /// Ensure the Starknet account is deployed (lazy deploy via paymaster if needed).
    /// No-op if already deployed or if the submitter does not support sponsored deploy.
    async fn ensure_deployed(
        &self,
        account: &ActiveAccount,
        config: &NetworkConfig,
        rpc: &RpcClient,
    ) -> Result<(), WalletError>;

    /// Deploy the Starknet account as a standalone operation (no invoke).
    /// Returns `Ok(Some(tx_hash))` if a deployment was submitted, `Ok(None)` if already deployed.
    async fn deploy_account(
        &self,
        account: &ActiveAccount,
        config: &NetworkConfig,
        rpc: &RpcClient,
    ) -> Result<Option<String>, WalletError>;

    async fn submit(
        &self,
        account: &ActiveAccount,
        calls: Vec<krusty_kms_client::starknet_rust::core::types::Call>,
    ) -> Result<String, WalletError>;
}

// ── PaymasterSubmitter ──────────────────────────────────────

/// Submits transactions via the AVNU paymaster (gasless/gasfree).
/// When the account needs deployment, `ensure_deployed` stores the deployment data
/// and `submit` uses SNIP-29 `deploy_and_invoke` to deploy + invoke atomically.
pub struct PaymasterSubmitter {
    paymaster: PaymasterClient,
    /// Pending deployment data set by `ensure_deployed`, consumed by `submit`.
    pending_deploy: std::sync::Mutex<Option<serde_json::Value>>,
}

impl PaymasterSubmitter {
    pub fn new(paymaster_url: &str, api_key: Option<&str>) -> Self {
        Self {
            paymaster: PaymasterClient::new(paymaster_url, api_key),
            pending_deploy: std::sync::Mutex::new(None),
        }
    }
}

/// Serialize a starknet-rs Call to SNIP-29 JSON format for the AVNU paymaster.
fn call_to_json(call: &krusty_kms_client::starknet_rust::core::types::Call) -> serde_json::Value {
    serde_json::json!({
        "to": format!("{:#066x}", call.to),
        "selector": format!("{:#066x}", call.selector),
        "calldata": call.calldata.iter().map(|f| format!("{:#066x}", f)).collect::<Vec<_>>(),
    })
}

#[async_trait::async_trait]
impl TransactionSubmitter for PaymasterSubmitter {
    async fn ensure_deployed(
        &self,
        account: &ActiveAccount,
        config: &NetworkConfig,
        rpc: &RpcClient,
    ) -> Result<(), WalletError> {
        let class_hash = Felt::from_hex(&config.account_class_hash)
            .map_err(|e| WalletError::Kms(format!("invalid account class hash: {e}")))?;
        if rpc
            .is_account_deployed(&account.starknet_address, &class_hash)
            .await?
        {
            return Ok(());
        }
        // Store deployment data — the next `submit` call will use `deploy_and_invoke`
        // to deploy the account and execute calls atomically.
        // ArgentX v0.4 constructor calldata: [0 (Starknet signer variant), pubkey, 1 (no guardian)]
        let deployment = serde_json::json!({
            "address": format!("{:#066x}", account.starknet_address),
            "class_hash": format!("{:#066x}", class_hash),
            "salt": "0x0",
            "calldata": ["0x0", format!("{:#066x}", account.starknet_public_key), "0x1"],
            "version": 1,
        });
        *self.pending_deploy.lock().unwrap() = Some(deployment);
        Ok(())
    }

    async fn deploy_account(
        &self,
        account: &ActiveAccount,
        config: &NetworkConfig,
        rpc: &RpcClient,
    ) -> Result<Option<String>, WalletError> {
        let class_hash = Felt::from_hex(&config.account_class_hash)
            .map_err(|e| WalletError::Kms(format!("invalid account class hash: {e}")))?;
        if rpc
            .is_account_deployed(&account.starknet_address, &class_hash)
            .await?
        {
            return Ok(None);
        }

        // Build deployment data (ArgentX v0.4 constructor)
        let address_hex = format!("{:#066x}", account.starknet_address);
        let deployment = serde_json::json!({
            "address": &address_hex,
            "class_hash": format!("{:#066x}", class_hash),
            "salt": "0x0",
            "calldata": ["0x0", format!("{:#066x}", account.starknet_public_key), "0x1"],
            "version": 1,
        });

        // Use deploy_and_invoke with a harmless ERC-20 approve(spender, 0) call.
        // Standalone "deploy" isn't supported by SNIP-29, so we need at least one invoke call.
        let erc20_addr = Felt::from_hex(&config.token_contract)
            .map_err(|e| WalletError::Kms(format!("invalid token contract: {e}")))?;
        let tongo_addr = Felt::from_hex(&config.tongo_contract)
            .map_err(|e| WalletError::Kms(format!("invalid tongo contract: {e}")))?;

        let approve_call = krusty_kms_client::build_erc20_approve(
            erc20_addr,
            tongo_addr,
            0, // transfer 0 tokens — harmless no-op
        )
        .map_err(|e| WalletError::Kms(e.to_string()))?;

        let calls_json = serde_json::Value::Array(vec![call_to_json(&approve_call)]);

        let build_resp = self
            .paymaster
            .build_deploy_and_invoke(&deployment, &address_hex, calls_json)
            .await?;

        let msg_hash = signing::compute_outside_execution_hash(
            &build_resp.typed_data,
            &account.starknet_address,
        )?;
        let (r, s) =
            signing::sign_message_hash(&msg_hash, &account.starknet_private_key)?;

        let r_hex = format!("{:#066x}", r);
        let s_hex = format!("{:#066x}", s);
        let exec_resp = self
            .paymaster
            .execute_deploy_and_invoke(
                &deployment,
                &address_hex,
                &build_resp.typed_data,
                &[r_hex, s_hex],
            )
            .await?;

        Ok(Some(exec_resp.transaction_hash))
    }

    async fn submit(
        &self,
        account: &ActiveAccount,
        calls: Vec<krusty_kms_client::starknet_rust::core::types::Call>,
    ) -> Result<String, WalletError> {
        let address_hex = format!("{:#066x}", account.starknet_address);

        let calls_json: Vec<serde_json::Value> = calls.iter().map(call_to_json).collect();
        let calls_json = serde_json::Value::Array(calls_json);

        // Check if account needs deployment (set by ensure_deployed).
        let pending_deploy = self.pending_deploy.lock().unwrap().take();

        if let Some(ref deployment) = pending_deploy {
            // Atomic deploy + invoke via SNIP-29 deploy_and_invoke.
            let build_resp = self
                .paymaster
                .build_deploy_and_invoke(deployment, &address_hex, calls_json)
                .await?;

            let msg_hash = signing::compute_outside_execution_hash(
                &build_resp.typed_data,
                &account.starknet_address,
            )?;
            let (r, s) =
                signing::sign_message_hash(&msg_hash, &account.starknet_private_key)?;

            let r_hex = format!("{:#066x}", r);
            let s_hex = format!("{:#066x}", s);
            // ArgentX v0.4 concise signature: [r, s] (contract reads pubkey from storage)
            let exec_resp = self
                .paymaster
                .execute_deploy_and_invoke(
                    deployment,
                    &address_hex,
                    &build_resp.typed_data,
                    &[r_hex, s_hex],
                )
                .await?;

            Ok(exec_resp.transaction_hash)
        } else {
            // Normal invoke flow (account already deployed).
            let build_resp = self
                .paymaster
                .build_typed_data(&address_hex, calls_json)
                .await?;

            let msg_hash = signing::compute_outside_execution_hash(
                &build_resp.typed_data,
                &account.starknet_address,
            )?;

            let (r, s) =
                signing::sign_message_hash(&msg_hash, &account.starknet_private_key)?;

            let r_hex = format!("{:#066x}", r);
            let s_hex = format!("{:#066x}", s);
            // ArgentX v0.4 concise signature: [r, s] (contract reads pubkey from storage)
            let exec_resp = self
                .paymaster
                .execute_invoke(&address_hex, &build_resp.typed_data, &[r_hex, s_hex])
                .await?;

            Ok(exec_resp.transaction_hash)
        }
    }
}

// ── DirectSubmitter (devnet only) ───────────────────────────

#[cfg(feature = "devnet")]
pub struct DirectSubmitter {
    rpc_url: String,
    chain_id: String,
}

#[cfg(feature = "devnet")]
impl DirectSubmitter {
    pub fn new(rpc_url: &str, chain_id: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            chain_id: chain_id.to_string(),
        }
    }
}

#[cfg(feature = "devnet")]
#[async_trait::async_trait]
impl TransactionSubmitter for DirectSubmitter {
    async fn ensure_deployed(
        &self,
        _account: &ActiveAccount,
        _config: &NetworkConfig,
        _rpc: &RpcClient,
    ) -> Result<(), WalletError> {
        // Devnet tests deploy the account explicitly in the fixture; no lazy deploy.
        Ok(())
    }

    async fn deploy_account(
        &self,
        _account: &ActiveAccount,
        _config: &NetworkConfig,
        _rpc: &RpcClient,
    ) -> Result<Option<String>, WalletError> {
        // Devnet tests deploy the account explicitly in the fixture.
        Ok(None)
    }

    async fn submit(
        &self,
        account: &ActiveAccount,
        calls: Vec<krusty_kms_client::starknet_rust::core::types::Call>,
    ) -> Result<String, WalletError> {
        use krusty_kms_client::starknet_rust::accounts::{
            Account, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount,
        };
        use krusty_kms_client::starknet_rust::core::types::Felt;
        use krusty_kms_client::starknet_rust::providers::jsonrpc::HttpTransport;
        use krusty_kms_client::starknet_rust::providers::JsonRpcClient;
        use krusty_kms_client::starknet_rust::signers::{LocalWallet, SigningKey};

        let url = url::Url::parse(&self.rpc_url)
            .map_err(|e| WalletError::Rpc(format!("invalid RPC URL: {e}")))?;
        let provider = JsonRpcClient::new(HttpTransport::new(url));

        let sk_felt = Felt::from_bytes_be(&account.starknet_private_key.to_bytes_be());
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(sk_felt));

        let address = Felt::from_bytes_be(&account.starknet_address.to_bytes_be());

        let chain_id = {
            let bytes = self.chain_id.as_bytes();
            let mut buf = [0u8; 32];
            let start = 32usize.saturating_sub(bytes.len());
            buf[start..].copy_from_slice(bytes);
            Felt::from_bytes_be(&buf)
        };

        let soa = SingleOwnerAccount::new(
            provider,
            signer,
            address,
            chain_id,
            ExecutionEncoding::New,
        );

        // Fetch nonce to verify connectivity (also cached by the account)
        let _nonce: Felt = soa
            .get_nonce()
            .await
            .map_err(|e| WalletError::Rpc(format!("get nonce: {e}")))?;

        let execution = soa.execute_v3(calls);

        let result = execution
            .send()
            .await
            .map_err(|e| WalletError::Rpc(format!("execute_v3: {e}")))?;

        Ok(format!("{:#066x}", result.transaction_hash))
    }
}

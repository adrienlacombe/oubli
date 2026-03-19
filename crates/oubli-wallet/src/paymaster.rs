use crate::error::WalletError;
use std::sync::atomic::{AtomicU64, Ordering};

/// AVNU paymaster client implementing SNIP-29 (JSON-RPC 2.0).
///
/// See <https://docs.avnu.fi/docs/paymaster/index> and
/// <https://github.com/starknet-io/SNIPs/blob/main/SNIPS/snip-29.md>.
pub struct PaymasterClient {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
    request_id: AtomicU64,
}

/// Response from `paymaster_buildTransaction` for invoke transactions.
#[derive(Debug, Clone)]
pub struct BuildInvokeResponse {
    /// SNIP-12 typed data for the outside execution — must be signed by the account.
    pub typed_data: serde_json::Value,
}

/// Response from `paymaster_executeTransaction`.
#[derive(Debug, Clone)]
pub struct ExecuteResponse {
    /// On-chain transaction hash.
    pub transaction_hash: String,
}

impl PaymasterClient {
    pub fn new(base_url: &str, api_key: Option<&str>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.map(String::from),
            client: reqwest::Client::new(),
            request_id: AtomicU64::new(1),
        }
    }

    /// Build the `fee_mode` object for SNIP-29 USER_PARAMETERS.
    fn fee_mode(&self) -> serde_json::Value {
        if self.api_key.is_some() {
            serde_json::json!({ "mode": "sponsored" })
        } else {
            // Default mode — pay gas in STRK.
            serde_json::json!({
                "mode": "default",
                "gas_token": "0x04718f5a0fc34cc1af16a1cdee98ffb20c31f5cd61d6ab07201858f4287c938d"
            })
        }
    }

    /// SNIP-29 USER_PARAMETERS sent with every build/execute call.
    fn user_parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "version": "0x1",
            "fee_mode": self.fee_mode(),
        })
    }

    /// Issue a JSON-RPC 2.0 call and return the `result` field.
    async fn jsonrpc_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WalletError> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut req = self.client.post(&self.base_url).json(&body);
        if let Some(ref key) = self.api_key {
            req = req.header("x-paymaster-api-key", key.as_str());
        }

        let resp = req
            .send()
            .await
            .map_err(|e| WalletError::Paymaster(format!("{method}: {e}")))?;

        let text = resp
            .text()
            .await
            .map_err(|e| WalletError::Paymaster(format!("{method} response: {e}")))?;

        let rpc_resp: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            WalletError::Paymaster(format!(
                "{method} decode: {e}; body: {}",
                if text.len() > 500 {
                    format!("{}...", &text[..500])
                } else {
                    text.clone()
                }
            ))
        })?;

        if let Some(error) = rpc_resp.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            let data = error.get("data").map(|d| d.to_string());
            let err_msg = match data {
                Some(d) if !d.is_empty() && d != "null" => format!(
                    "{method} JSON-RPC error: code={code}, message=\"{message}\", data={d}"
                ),
                _ => format!(
                    "{method} JSON-RPC error: code={code}, message=\"{message}\""
                ),
            };
            return Err(WalletError::Paymaster(err_msg));
        }

        rpc_resp
            .get("result")
            .cloned()
            .ok_or_else(|| WalletError::Paymaster(format!("{method}: missing 'result' in response")))
    }

    // ── Build ──────────────────────────────────────────────────

    /// Build typed data for an invoke transaction (outside execution).
    pub async fn build_typed_data(
        &self,
        user_address: &str,
        calls: serde_json::Value,
    ) -> Result<BuildInvokeResponse, WalletError> {
        let params = serde_json::json!({
            "transaction": {
                "type": "invoke",
                "invoke": {
                    "user_address": user_address,
                    "calls": calls,
                }
            },
            "parameters": self.user_parameters(),
        });

        let result = self
            .jsonrpc_call("paymaster_buildTransaction", params)
            .await?;

        let typed_data = result.get("typed_data").cloned().ok_or_else(|| {
            WalletError::Paymaster("buildTransaction(invoke): missing typed_data".into())
        })?;

        Ok(BuildInvokeResponse { typed_data })
    }

    /// Build deployment data for a sponsored DEPLOY_ACCOUNT.
    /// Returns the deployment object to pass to [`execute_deploy`].
    pub async fn build_deploy(
        &self,
        account_address: &str,
        class_hash: &str,
        salt: &str,
        calldata: &[String],
    ) -> Result<serde_json::Value, WalletError> {
        let deployment = serde_json::json!({
            "address": account_address,
            "class_hash": class_hash,
            "salt": salt,
            "calldata": calldata,
            "version": 1,
        });

        let params = serde_json::json!({
            "transaction": {
                "type": "deploy",
                "deployment": &deployment,
            },
            "parameters": self.user_parameters(),
        });

        // The build step lets the paymaster prepare/validate the deployment.
        let _result = self
            .jsonrpc_call("paymaster_buildTransaction", params)
            .await?;

        Ok(deployment)
    }

    // ── Execute ────────────────────────────────────────────────

    /// Submit a signed invoke transaction through the paymaster.
    pub async fn execute_invoke(
        &self,
        user_address: &str,
        typed_data: &serde_json::Value,
        signature: &[String],
    ) -> Result<ExecuteResponse, WalletError> {
        let params = serde_json::json!({
            "transaction": {
                "type": "invoke",
                "invoke": {
                    "user_address": user_address,
                    "typed_data": typed_data,
                    "signature": signature,
                }
            },
            "parameters": self.user_parameters(),
        });

        let result = self
            .jsonrpc_call("paymaster_executeTransaction", params)
            .await?;

        let tx_hash = result
            .get("transaction_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                WalletError::Paymaster(
                    "executeTransaction(invoke): missing transaction_hash".into(),
                )
            })?
            .to_string();

        Ok(ExecuteResponse {
            transaction_hash: tx_hash,
        })
    }

    /// Submit an account deployment through the paymaster.
    pub async fn execute_deploy(
        &self,
        deployment: &serde_json::Value,
    ) -> Result<ExecuteResponse, WalletError> {
        let params = serde_json::json!({
            "transaction": {
                "type": "deploy",
                "deployment": deployment,
            },
            "parameters": self.user_parameters(),
        });

        let result = self
            .jsonrpc_call("paymaster_executeTransaction", params)
            .await?;

        let tx_hash = result
            .get("transaction_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                WalletError::Paymaster(
                    "executeTransaction(deploy): missing transaction_hash".into(),
                )
            })?
            .to_string();

        Ok(ExecuteResponse {
            transaction_hash: tx_hash,
        })
    }

    // ── Deploy + Invoke (atomic) ──────────────────────────────

    /// Build typed data for an atomic deploy-and-invoke transaction.
    /// Used when the account is not yet deployed and needs to execute calls in the same operation.
    pub async fn build_deploy_and_invoke(
        &self,
        deployment: &serde_json::Value,
        user_address: &str,
        calls: serde_json::Value,
    ) -> Result<BuildInvokeResponse, WalletError> {
        let params = serde_json::json!({
            "transaction": {
                "type": "deploy_and_invoke",
                "deployment": deployment,
                "invoke": {
                    "user_address": user_address,
                    "calls": calls,
                }
            },
            "parameters": self.user_parameters(),
        });

        let result = self
            .jsonrpc_call("paymaster_buildTransaction", params)
            .await?;

        let typed_data = result.get("typed_data").cloned().ok_or_else(|| {
            WalletError::Paymaster(
                "buildTransaction(deploy_and_invoke): missing typed_data".into(),
            )
        })?;

        Ok(BuildInvokeResponse { typed_data })
    }

    /// Submit a signed deploy-and-invoke transaction through the paymaster.
    pub async fn execute_deploy_and_invoke(
        &self,
        deployment: &serde_json::Value,
        user_address: &str,
        typed_data: &serde_json::Value,
        signature: &[String],
    ) -> Result<ExecuteResponse, WalletError> {
        let params = serde_json::json!({
            "transaction": {
                "type": "deploy_and_invoke",
                "deployment": deployment,
                "invoke": {
                    "user_address": user_address,
                    "typed_data": typed_data,
                    "signature": signature,
                }
            },
            "parameters": self.user_parameters(),
        });

        let result = self
            .jsonrpc_call("paymaster_executeTransaction", params)
            .await?;

        let tx_hash = result
            .get("transaction_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                WalletError::Paymaster(
                    "executeTransaction(deploy_and_invoke): missing transaction_hash".into(),
                )
            })?
            .to_string();

        Ok(ExecuteResponse {
            transaction_hash: tx_hash,
        })
    }
}

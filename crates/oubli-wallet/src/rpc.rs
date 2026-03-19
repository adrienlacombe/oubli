use std::sync::Arc;

use krusty_kms_client::TongoContract;
use starknet_types_core::curve::ProjectivePoint;
use starknet_types_core::felt::Felt;

use crate::config::NetworkConfig;
use crate::core::ActivityEvent;
use crate::denomination::{format_sats_display, tongo_units_to_sats};
use crate::error::WalletError;

/// Wrapper around `starknet-client` for querying on-chain state.
pub struct RpcClient {
    rpc_url: String,
    contract: TongoContract,
    tongo_contract_hex: String,
}

/// Decrypted account balances for display.
#[derive(Debug, Clone)]
pub struct DecryptedBalance {
    pub balance: u128,
    pub pending: u128,
    pub nonce: Felt,
}

impl RpcClient {
    /// Create a new RPC client from network config.
    pub fn new(config: &NetworkConfig) -> Result<Self, WalletError> {
        let provider = krusty_kms_client::create_provider(&config.rpc_url)
            .map_err(|e| WalletError::Rpc(e.to_string()))?;

        let tongo_address = Felt::from_hex(&config.tongo_contract)
            .map_err(|e| WalletError::Rpc(format!("invalid tongo contract address: {e}")))?;

        // starknet-client uses starknet's CoreFelt (re-exported)
        let core_felt = krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(&tongo_address.to_bytes_be());

        let provider = Arc::new(provider);
        let contract = TongoContract::new(provider, core_felt);
        Ok(Self {
            rpc_url: config.rpc_url.clone(),
            contract,
            tongo_contract_hex: config.tongo_contract.clone(),
        })
    }

    /// Returns true if any contract is deployed at `account_address`.
    /// The `_expected_class_hash` parameter is retained for API compatibility but not checked,
    /// since accounts may be upgraded to a different class (e.g. OZ → Argent).
    pub async fn is_account_deployed(
        &self,
        account_address: &Felt,
        _expected_class_hash: &Felt,
    ) -> Result<bool, WalletError> {
        let addr_hex = format!("{:#066x}", account_address);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "starknet_getClassHashAt",
            "params": ["latest", addr_hex]
        });
        let client = reqwest::Client::new();
        let resp = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WalletError::Rpc(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| WalletError::Rpc(e.to_string()))?;
        if !status.is_success() {
            return Err(WalletError::Rpc(format!("getClassHashAt failed ({status}): {text}")));
        }
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| WalletError::Rpc(e.to_string()))?;
        match json.get("result") {
            Some(serde_json::Value::String(s)) => {
                let at_address = Felt::from_hex(s).unwrap_or(Felt::ZERO);
                Ok(at_address != Felt::ZERO)
            }
            _ => Ok(false),
        }
    }

    /// Fetch on-chain state and decrypt balances using the private key.
    pub async fn fetch_decrypted_balance(
        &self,
        _private_key: &Felt,
        owner_public_key: &starknet_types_core::curve::ProjectivePoint,
    ) -> Result<DecryptedBalance, WalletError> {
        let state = self
            .contract
            .get_state(owner_public_key)
            .await
            .map_err(|e| WalletError::Rpc(e.to_string()))?;

        let balance = krusty_kms_client::decrypt_cipher_balance(_private_key, &state.balance)
            .map_err(|e| WalletError::Rpc(e.to_string()))?;
        let pending = krusty_kms_client::decrypt_cipher_balance(_private_key, &state.pending)
            .map_err(|e| WalletError::Rpc(e.to_string()))?;

        Ok(DecryptedBalance {
            balance,
            pending,
            nonce: state.nonce,
        })
    }

    /// Check whether a transaction has been accepted on-chain (ACCEPTED_ON_L2 or ACCEPTED_ON_L1).
    pub async fn is_tx_confirmed(&self, tx_hash: &str) -> Result<bool, WalletError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "starknet_getTransactionReceipt",
            "params": [tx_hash]
        });
        let client = reqwest::Client::new();
        let resp = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WalletError::Rpc(e.to_string()))?;
        let text = resp.text().await.map_err(|e| WalletError::Rpc(e.to_string()))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| WalletError::Rpc(e.to_string()))?;
        if let Some(result) = json.get("result") {
            if let Some(status) = result.get("finality_status").and_then(|s| s.as_str()) {
                return Ok(status == "ACCEPTED_ON_L2" || status == "ACCEPTED_ON_L1");
            }
        }
        Ok(false)
    }

    /// Query the ERC-20 `balance_of` for `account_address` on `token_address`.
    /// Returns the balance as a u128 (low limb of the u256 response).
    pub async fn get_erc20_balance(
        &self,
        token_address: &Felt,
        account_address: &Felt,
    ) -> Result<u128, WalletError> {
        let token_hex = format!("{:#066x}", token_address);
        let account_hex = format!("{:#066x}", account_address);
        // Well-known selector for `balance_of`
        let selector = "0x02e4263afad30923c891518314c3c95dbe830a16874e8abc5777a9a20b54c76e";
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "starknet_call",
            "params": [{
                "contract_address": token_hex,
                "entry_point_selector": selector,
                "calldata": [account_hex]
            }, "latest"]
        });
        let client = reqwest::Client::new();
        let resp = client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WalletError::Rpc(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| WalletError::Rpc(e.to_string()))?;
        if !status.is_success() {
            return Err(WalletError::Rpc(format!("starknet_call balance_of failed ({status}): {text}")));
        }
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| WalletError::Rpc(e.to_string()))?;
        match json.get("result") {
            Some(serde_json::Value::Array(arr)) if !arr.is_empty() => {
                // Response is [low, high] as hex strings (u256 split)
                let low_hex = arr[0].as_str().unwrap_or("0x0");
                let low = Felt::from_hex(low_hex).unwrap_or(Felt::ZERO);
                let low_bytes = low.to_bytes_be();
                let mut u128_bytes = [0u8; 16];
                u128_bytes.copy_from_slice(&low_bytes[16..32]);
                Ok(u128::from_be_bytes(u128_bytes))
            }
            _ => Ok(0),
        }
    }

    /// Get a reference to the underlying contract.
    pub fn contract(&self) -> &TongoContract {
        &self.contract
    }

    /// Fetch the most recent activity events (up to 10, newest first).
    ///
    /// Uses raw HTTP `starknet_getEvents` calls instead of starknet-rs's `get_events`
    /// because starknet-rs 0.17 expects RPC spec v0.8 fields (`transaction_index`,
    /// `event_index`) that many RPC nodes don't return yet.
    pub async fn get_recent_activity(
        &self,
        pub_key: &ProjectivePoint,
        private_key: Option<&Felt>,
        cached: &[ActivityEvent],
    ) -> Result<Vec<ActivityEvent>, WalletError> {
        let affine = pub_key
            .to_affine()
            .map_err(|_| WalletError::Rpc("invalid public key".into()))?;
        // Use {:#x} (no zero-padding) to match the RPC node's hex format in key filters.
        let pk_x = format!("{:#x}", affine.x());
        let pk_y = format!("{:#x}", affine.y());

        let client = reqwest::Client::new();
        let mut new_events: Vec<ActivityEvent> = Vec::new();

        // Use the highest cached block number as from_block to avoid re-scanning
        // all history. We re-fetch from that block (inclusive) to catch any events
        // in the same block that we might have missed.
        let from_block = cached.iter().map(|e| e.block_number).max();

        // Compute event selectors via starknet_keccak.
        let fund_sel = starknet_keccak_hex(b"FundEvent");
        let outside_fund_sel = starknet_keccak_hex(b"OutsideFundEvent");
        let withdraw_sel = starknet_keccak_hex(b"WithdrawEvent");
        let ragequit_sel = starknet_keccak_hex(b"RagequitEvent");
        let transfer_sel = starknet_keccak_hex(b"TransferEvent");

        // Query 1: Events where pub_key is at keys[1..2] (Fund, OutsideFund,
        //          Withdraw, Ragequit, TransferIn).
        let selectors_q1 = vec![
            &fund_sel, &outside_fund_sel, &withdraw_sel, &ragequit_sel,
            &transfer_sel,
        ];
        let keys_q1 = serde_json::json!([
            selectors_q1,
            [&pk_x],
            [&pk_y]
        ]);
        let events_q1 = self.fetch_raw_events(&client, &keys_q1, from_block).await?;

        for ev in &events_q1 {
            if let Some(activity) = raw_event_to_activity(ev, pub_key, private_key) {
                new_events.push(activity);
            }
        }

        // Query 2: TransferOut events where pub_key is sender (keys[3..4]).
        let keys_q2 = serde_json::json!([
            [&transfer_sel],
            [],
            [],
            [&pk_x],
            [&pk_y]
        ]);
        let events_q2 = self.fetch_raw_events(&client, &keys_q2, from_block).await?;

        for ev in &events_q2 {
            if let Some(activity) = raw_event_to_activity(ev, pub_key, private_key) {
                if !new_events.iter().any(|a| a.tx_hash == activity.tx_hash && a.event_type == activity.event_type) {
                    new_events.push(activity);
                }
            }
        }

        // Merge: prefer freshly-fetched events over cached (they may have
        // newly-decrypted amounts). Then add any cached events not re-fetched.
        let mut merged = new_events.clone();
        for ev in cached {
            if !merged.iter().any(|a| a.tx_hash == ev.tx_hash && a.event_type == ev.event_type) {
                merged.push(ev.clone());
            }
        }

        // Sort newest first, keep last 20. Treat block_number 0 as pending (newest).
        merged.sort_by(|a, b| {
            match (a.block_number, b.block_number) {
                (0, 0) => std::cmp::Ordering::Equal,
                (0, _) => std::cmp::Ordering::Less,  // a is pending → comes first
                (_, 0) => std::cmp::Ordering::Greater,
                _ => b.block_number.cmp(&a.block_number),
            }
        });
        merged.truncate(20);
        Ok(merged)
    }

    /// Low-level: fetch raw events from `starknet_getEvents` using HTTP POST.
    /// Returns parsed JSON event objects. Paginates through all results.
    async fn fetch_raw_events(
        &self,
        client: &reqwest::Client,
        keys: &serde_json::Value,
        from_block: Option<u64>,
    ) -> Result<Vec<serde_json::Value>, WalletError> {
        let mut all_events = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut filter = serde_json::json!({
                "to_block": "latest",
                "address": &self.tongo_contract_hex,
                "keys": keys,
                "chunk_size": 100
            });
            if let Some(block) = from_block {
                filter["from_block"] = serde_json::json!({"block_number": block});
            }
            if let Some(ref token) = continuation_token {
                filter["continuation_token"] = serde_json::Value::String(token.clone());
            }

            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "starknet_getEvents",
                "params": { "filter": filter }
            });

            // Retry with exponential backoff on rate-limit (429) or transient errors.
            let mut json: serde_json::Value = serde_json::Value::Null;
            for attempt in 0..4u32 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(500 * 2u64.pow(attempt))).await;
                }
                let resp = client
                    .post(&self.rpc_url)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| WalletError::Rpc(e.to_string()))?;
                let status = resp.status();
                let text = resp.text().await.map_err(|e| WalletError::Rpc(e.to_string()))?;
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    if attempt == 3 {
                        return Err(WalletError::Rpc("rate limited after 4 attempts".into()));
                    }
                    continue;
                }
                match serde_json::from_str(&text) {
                    Ok(v) => { json = v; break; }
                    Err(e) if attempt < 3 => continue,
                    Err(e) => return Err(WalletError::Rpc(format!("RPC error: {e}"))),
                }
            }

            if let Some(err) = json.get("error") {
                return Err(WalletError::Rpc(format!(
                    "starknet_getEvents error: {}",
                    err
                )));
            }

            let result = json
                .get("result")
                .ok_or_else(|| WalletError::Rpc("missing result in getEvents response".into()))?;

            if let Some(events) = result.get("events").and_then(|e| e.as_array()) {
                all_events.extend(events.iter().cloned());
            }

            match result.get("continuation_token").and_then(|t| t.as_str()) {
                Some(token) => continuation_token = Some(token.to_string()),
                None => break,
            }
        }

        Ok(all_events)
    }
}

// ── Helpers ─────────────────────────────────────────────────────

/// Compute the starknet_keccak hash of a name and return as "0x..." hex string.
/// Uses no zero-padding to match the RPC node's hex format.
fn starknet_keccak_hex(name: &[u8]) -> String {
    let felt = krusty_kms_client::starknet_rust::core::utils::starknet_keccak(name);
    format!("{:#x}", felt)
}

/// Convert a hex felt string from JSON to a u128 value.
fn hex_to_u128(hex: &str) -> u128 {
    let felt = Felt::from_hex(hex).unwrap_or(Felt::ZERO);
    let bytes = felt.to_bytes_be();
    let mut buf = [0u8; 16];
    buf.copy_from_slice(&bytes[16..32]);
    u128::from_be_bytes(buf)
}

/// Parse a raw JSON event into an `ActivityEvent`.
///
/// Returns `None` for internal events (BalanceDeclared, TransferDeclared)
/// and for events that can't be parsed.
fn raw_event_to_activity(
    ev: &serde_json::Value,
    user_pub_key: &ProjectivePoint,
    user_private_key: Option<&Felt>,
) -> Option<ActivityEvent> {
    let keys = ev.get("keys")?.as_array()?;
    let data = ev.get("data")?.as_array()?;
    let tx_hash = ev.get("transaction_hash")?.as_str()?.to_string();
    let block_number = ev.get("block_number").and_then(|b| b.as_u64()).unwrap_or(0);

    let selector = keys.first()?.as_str()?;

    let fund_sel = starknet_keccak_hex(b"FundEvent");
    let outside_fund_sel = starknet_keccak_hex(b"OutsideFundEvent");
    let withdraw_sel = starknet_keccak_hex(b"WithdrawEvent");
    let ragequit_sel = starknet_keccak_hex(b"RagequitEvent");
    let rollover_sel = starknet_keccak_hex(b"RolloverEvent");
    let transfer_sel = starknet_keccak_hex(b"TransferEvent");

    let (event_type, amount_sats) = if selector == fund_sel {
        // Current format: keys=[selector, to.x, to.y, nonce], data=[amount]
        let amount = data.first().and_then(|v| v.as_str()).map(hex_to_u128)?;
        ("Fund".to_string(), Some(format_sats_display(&tongo_units_to_sats(amount as u64))))
    } else if selector == outside_fund_sel {
        // Current format: keys=[selector, to.x, to.y], data=[from, amount] or data=[amount]
        // Try last data element as amount (works for both old and new formats).
        let amount = data.last().and_then(|v| v.as_str()).map(hex_to_u128)?;
        ("Fund".to_string(), Some(format_sats_display(&tongo_units_to_sats(amount as u64))))
    } else if selector == withdraw_sel {
        // Current format: keys=[selector, from.x, from.y, nonce], data=[amount, to] or data=[amount]
        let amount = data.first().and_then(|v| v.as_str()).map(hex_to_u128)?;
        ("Withdraw".to_string(), Some(format_sats_display(&tongo_units_to_sats(amount as u64))))
    } else if selector == ragequit_sel {
        // Current format: keys=[selector, from.x, from.y, nonce], data=[amount, to] or data=[amount]
        let amount = data.first().and_then(|v| v.as_str()).map(hex_to_u128)?;
        ("Ragequit".to_string(), Some(format_sats_display(&tongo_units_to_sats(amount as u64))))
    } else if selector == rollover_sel {
        // Internal settlement — hide from user activity.
        return None;
    } else if selector == transfer_sel {
        // keys: [selector, to.x, to.y, from.x, from.y, nonce]
        // data: [transfer_balance(4), transfer_balance_self(4), hint_transfer(6), hint_leftover(6)]
        let from_x = keys.get(3).and_then(|v| v.as_str());
        let from_y = keys.get(4).and_then(|v| v.as_str());

        let is_sender = match (from_x, from_y) {
            (Some(fx), Some(fy)) => {
                if let Ok(affine) = user_pub_key.to_affine() {
                    let ux = format!("{:#x}", affine.x());
                    let uy = format!("{:#x}", affine.y());
                    fx == ux && fy == uy
                } else {
                    false
                }
            }
            _ => false,
        };

        let label = if is_sender { "TransferOut" } else { "TransferIn" };

        // Try to decrypt transfer amount from hint_transfer (data[8..14]).
        let amount_sats = user_private_key.and_then(|priv_key| {
            // Counterparty: recipient (keys[1..2]) if sender, sender (keys[3..4]) if recipient.
            let (cx, cy) = if is_sender {
                (keys.get(1)?.as_str()?, keys.get(2)?.as_str()?)
            } else {
                (keys.get(3)?.as_str()?, keys.get(4)?.as_str()?)
            };
            let counterparty = parse_point_from_hex(cx, cy)?;
            let amount = decrypt_transfer_hint(data, 8, priv_key, &counterparty)?;
            Some(format_sats_display(&tongo_units_to_sats(amount as u64)))
        });

        (label.to_string(), amount_sats)
    } else {
        // BalanceDeclared, TransferDeclared, or unknown — skip
        return None;
    };

    Some(ActivityEvent {
        event_type,
        amount_sats,
        tx_hash,
        block_number,
    })
}

/// Parse a projective point from two hex coordinate strings.
fn parse_point_from_hex(x_hex: &str, y_hex: &str) -> Option<ProjectivePoint> {
    let x = Felt::from_hex(x_hex).ok()?;
    let y = Felt::from_hex(y_hex).ok()?;
    let affine = starknet_types_core::curve::AffinePoint::new(x, y).ok()?;
    Some(krusty_kms_crypto::StarkCurve::affine_to_projective(&affine))
}

/// Decrypt the AEBalance hint at `data[offset..offset+6]` using ECDH.
/// AEBalance layout: [ciphertext(4 felts), nonce(2 felts)].
fn decrypt_transfer_hint(
    data: &[serde_json::Value],
    offset: usize,
    private_key: &Felt,
    counterparty: &ProjectivePoint,
) -> Option<u128> {
    // Parse 4 ciphertext felts → 64 bytes
    let mut ciphertext = [0u8; 64];
    for i in 0..4 {
        let felt = Felt::from_hex(data.get(offset + i)?.as_str()?).ok()?;
        let bytes = felt.to_bytes_be();
        // Each felt is 32 bytes but we pack into 16 bytes (lower half)
        ciphertext[i * 16..(i + 1) * 16].copy_from_slice(&bytes[16..32]);
    }
    // Parse 2 nonce felts → 24 bytes.
    // Serialization pads 24-byte nonce to 32, then splits as u256 (low, high):
    //   data[offset+4] = nonce_low  = Felt(u128 from padded[16..32]) = nonce[16..24] + 0-padding
    //   data[offset+5] = nonce_high = Felt(u128 from padded[0..16])  = nonce[0..16]
    let mut nonce = [0u8; 24];
    let n_low = Felt::from_hex(data.get(offset + 4)?.as_str()?).ok()?;
    let n_high = Felt::from_hex(data.get(offset + 5)?.as_str()?).ok()?;
    nonce[0..16].copy_from_slice(&n_high.to_bytes_be()[16..32]);
    nonce[16..24].copy_from_slice(&n_low.to_bytes_be()[16..24]);

    let shared_secret = krusty_kms_sdk::crypto::derive_shared_secret(private_key, counterparty).ok()?;
    krusty_kms_sdk::crypto::decrypt_audit_hint(&ciphertext, &nonce, &shared_secret).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulate a TransferEvent: encrypt a known amount using the SDK, serialize
    /// it into JSON felts the same way the on-chain contract emits them, then
    /// verify that `raw_event_to_activity` correctly decrypts and displays it.
    #[test]
    fn test_transfer_event_amount_roundtrip() {
        // Two keypairs: sender and recipient.
        let sender_priv = Felt::from_hex("0xdead").unwrap();
        let sender_pub = krusty_kms_crypto::StarkCurve::mul_generator(&sender_priv);
        let recipient_priv = Felt::from_hex("0xbeef").unwrap();
        let recipient_pub = krusty_kms_crypto::StarkCurve::mul_generator(&recipient_priv);

        // Amount to transfer (in tongo units).
        let amount: u128 = 42_000;

        // Encrypt the hint using sender's priv key + recipient's pub key (ECDH).
        let shared_secret =
            krusty_kms_sdk::crypto::derive_shared_secret(&sender_priv, &recipient_pub).unwrap();
        let (ct, nonce) =
            krusty_kms_sdk::crypto::encrypt_audit_hint(amount, &shared_secret).unwrap();

        // Serialize ciphertext+nonce into 6 felts using the SDK serializer
        // (this is what the Cairo contract emits on-chain).
        let hint_felts =
            krusty_kms_sdk::serialization::serialize_ae_balance(&ct, &nonce).unwrap();
        assert_eq!(hint_felts.len(), 6);

        // Build the full data array for a TransferEvent:
        // [transfer_balance(4), transfer_balance_self(4), hint_transfer(6), hint_leftover(6)]
        // = 4 + 4 + 6 + 6 = 20 felts (nonce is in keys[5])
        let zero = Felt::ZERO;
        let mut data_felts: Vec<Felt> = Vec::new();
        for _ in 0..4 { data_felts.push(zero); } // transfer_balance (dummy)
        for _ in 0..4 { data_felts.push(zero); } // transfer_balance_self (dummy)
        data_felts.extend_from_slice(&hint_felts);  // hint_transfer at offset 8
        for _ in 0..6 { data_felts.push(zero); } // hint_leftover (dummy)

        // Convert to JSON values.
        let data_json: Vec<serde_json::Value> = data_felts
            .iter()
            .map(|f| serde_json::Value::String(format!("{:#x}", f)))
            .collect();

        // Build keys: [selector, to.x, to.y, from.x, from.y, nonce]
        let transfer_sel = starknet_keccak_hex(b"TransferEvent");
        let recipient_affine = recipient_pub.to_affine().unwrap();
        let sender_affine = sender_pub.to_affine().unwrap();
        let keys_json = vec![
            serde_json::Value::String(transfer_sel),
            serde_json::Value::String(format!("{:#x}", recipient_affine.x())),
            serde_json::Value::String(format!("{:#x}", recipient_affine.y())),
            serde_json::Value::String(format!("{:#x}", sender_affine.x())),
            serde_json::Value::String(format!("{:#x}", sender_affine.y())),
            serde_json::Value::String("0x1".to_string()), // nonce
        ];

        let tx_hash = "0xdeadbeef";
        let block_number = 12345u64;

        let ev = serde_json::json!({
            "keys": keys_json,
            "data": data_json,
            "transaction_hash": tx_hash,
            "block_number": block_number,
        });

        // --- Test as sender ---
        let activity = raw_event_to_activity(&ev, &sender_pub, Some(&sender_priv))
            .expect("should parse transfer event for sender");
        assert_eq!(activity.event_type, "TransferOut");
        assert!(
            activity.amount_sats.is_some(),
            "sender should be able to decrypt transfer amount"
        );
        assert_eq!(activity.tx_hash, tx_hash);
        assert_eq!(activity.block_number, block_number);

        // --- Test as recipient ---
        let activity = raw_event_to_activity(&ev, &recipient_pub, Some(&recipient_priv))
            .expect("should parse transfer event for recipient");
        assert_eq!(activity.event_type, "TransferIn");
        assert!(
            activity.amount_sats.is_some(),
            "recipient should be able to decrypt transfer amount"
        );
    }

    /// Verify that the nonce deserialization from felts exactly matches
    /// the original nonce bytes produced by the SDK serializer.
    #[test]
    fn test_nonce_felt_roundtrip() {
        // Use a nonce with all distinct bytes so any scrambling is obvious.
        let original_nonce: [u8; 24] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
        ];
        let ciphertext = [0u8; 64]; // dummy, we only care about nonce

        let felts =
            krusty_kms_sdk::serialization::serialize_ae_balance(&ciphertext, &original_nonce)
                .unwrap();
        // felts[4] = nonce_low, felts[5] = nonce_high

        // Convert to JSON hex strings and back, simulating the RPC path.
        let n_low_hex = format!("{:#x}", felts[4]);
        let n_high_hex = format!("{:#x}", felts[5]);
        let n_low = Felt::from_hex(&n_low_hex).unwrap();
        let n_high = Felt::from_hex(&n_high_hex).unwrap();

        // Deserialize using our logic.
        let mut recovered = [0u8; 24];
        recovered[0..16].copy_from_slice(&n_high.to_bytes_be()[16..32]);
        recovered[16..24].copy_from_slice(&n_low.to_bytes_be()[16..24]);

        assert_eq!(recovered, original_nonce, "nonce roundtrip must be lossless");
    }
}

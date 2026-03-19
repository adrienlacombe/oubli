use starknet_types_core::felt::Felt;

use crate::config::NetworkConfig;
use crate::core::ActiveAccount;
use crate::denomination::sats_to_tongo_units;
use crate::error::WalletError;
use crate::rpc::RpcClient;
use crate::submitter::TransactionSubmitter;

// ── Helpers ──────────────────────────────────────────────────

fn chain_id_felt(chain_id: &str) -> Felt {
    // Convert short string chain_id to Felt
    let bytes = chain_id.as_bytes();
    let mut buf = [0u8; 32];
    let start = 32usize.saturating_sub(bytes.len());
    buf[start..].copy_from_slice(bytes);
    Felt::from_bytes_be(&buf)
}

/// Default hint values when no audit is configured.
fn zero_hint() -> ([u8; 64], [u8; 24]) {
    ([0u8; 64], [0u8; 24])
}

/// Parse a public key hex string into (x, y) Felts.
/// Accepts "0x" + 128 hex-char format (64 per coordinate). Trims whitespace
/// and tolerates slight length variations via zero-padding or leading-zero trimming.
fn parse_pubkey_hex(hex: &str) -> Result<(Felt, Felt), String> {
    let trimmed = hex.trim();
    let cleaned = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    // Remove any non-hex characters (whitespace, newlines, etc.)
    let hex_only: String = cleaned.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex_only.len() < 2 || hex_only.len() > 140 {
        return Err(format!(
            "public key hex has unexpected length {} (expected ~128)",
            hex_only.len()
        ));
    }
    // Normalize to exactly 128 hex chars (64 per coordinate).
    let normalized = if hex_only.len() < 128 {
        format!("{:0>128}", hex_only)
    } else if hex_only.len() > 128 {
        let excess = hex_only.len() - 128;
        if hex_only[..excess].chars().all(|c| c == '0') {
            hex_only[excess..].to_string()
        } else {
            return Err(format!(
                "public key hex too long ({}) with non-zero leading chars",
                hex_only.len()
            ));
        }
    } else {
        hex_only
    };
    let x = Felt::from_hex(&format!("0x{}", &normalized[..64]))
        .map_err(|e| format!("invalid x: {e}"))?;
    let y = Felt::from_hex(&format!("0x{}", &normalized[64..]))
        .map_err(|e| format!("invalid y: {e}"))?;
    Ok((x, y))
}

/// Get cipher balance from account, converting CipherBalance → ElGamalCiphertext.
fn account_cipher_balance(
    account: &ActiveAccount,
) -> Result<krusty_kms_common::ElGamalCiphertext, WalletError> {
    let cb = account
        .cipher_balance
        .as_ref()
        .ok_or(WalletError::NoActiveAccount)?;
    Ok(krusty_kms_common::ElGamalCiphertext {
        l: cb.l.clone(),
        r: cb.r.clone(),
    })
}

// ── Fund ─────────────────────────────────────────────────────

pub async fn execute_fund(
    account: &mut ActiveAccount,
    amount_sats: &str,
    config: &NetworkConfig,
    rpc: &RpcClient,
    submitter: &dyn TransactionSubmitter,
) -> Result<String, WalletError> {
    let amount = sats_to_tongo_units(amount_sats)? as u128;

    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;
    let cipher = account_cipher_balance(account)?;

    let params = krusty_kms_sdk::operations::FundParams {
        amount,
        nonce: account.nonce,
        chain_id: chain_id_felt(&config.chain_id),
        tongo_address: tongo_addr,
        sender_address: account.starknet_address,
        auditor_pub_key: account.auditor_key.clone(),
        current_balance: cipher,
    };

    let proof =
        krusty_kms_sdk::fund(&account.tongo_account, params).map_err(|e| WalletError::Kms(e.to_string()))?;

    let (hint_ct, hint_nonce) = match &proof.audit {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => zero_hint(),
    };

    let erc20_addr =
        Felt::from_hex(&config.token_contract).map_err(|e| WalletError::Kms(e.to_string()))?;

    // Get rate from contract
    let rate = rpc
        .contract()
        .get_rate()
        .await
        .map_err(|e| WalletError::Rpc(e.to_string()))?;

    let (approve_call, fund_call) =
        krusty_kms_client::build_fund_calls(tongo_addr, erc20_addr, rate, &proof, &hint_ct, &hint_nonce)
            .map_err(|e| WalletError::Kms(e.to_string()))?;

    submitter
        .ensure_deployed(account, config, rpc)
        .await?;
    let tx_hash = submitter.submit(account, vec![approve_call, fund_call]).await?;

    // Optimistic update
    account.balance += amount;
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

// ── Rollover ─────────────────────────────────────────────────

pub async fn execute_rollover(
    account: &mut ActiveAccount,
    config: &NetworkConfig,
    _rpc: &RpcClient,
    submitter: &dyn TransactionSubmitter,
) -> Result<String, WalletError> {
    if account.pending == 0 {
        return Err(WalletError::InvalidState {
            expected: "pending > 0".into(),
            got: "pending == 0".into(),
        });
    }

    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;

    let params = krusty_kms_sdk::operations::RolloverParams {
        nonce: account.nonce,
        chain_id: chain_id_felt(&config.chain_id),
        tongo_address: tongo_addr,
        sender_address: account.starknet_address,
    };

    let proof = krusty_kms_sdk::rollover(&account.tongo_account, params)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    let (hint_ct, hint_nonce) = zero_hint();

    let rollover_call =
        krusty_kms_client::build_rollover_call(tongo_addr, &proof, &hint_ct, &hint_nonce)
            .map_err(|e| WalletError::Kms(e.to_string()))?;

    submitter
        .ensure_deployed(account, config, _rpc)
        .await?;
    let tx_hash = submitter.submit(account, vec![rollover_call]).await?;

    // Optimistic update: pending → balance
    account.balance += account.pending;
    account.pending = 0;
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

// ── Transfer ─────────────────────────────────────────────────

pub async fn execute_transfer(
    account: &mut ActiveAccount,
    amount_sats: &str,
    recipient_pub_key_hex: &str,
    config: &NetworkConfig,
    rpc: &RpcClient,
    submitter: &dyn TransactionSubmitter,
) -> Result<String, WalletError> {
    let amount = sats_to_tongo_units(amount_sats)? as u128;

    if account.balance < amount {
        return Err(WalletError::InsufficientBalance {
            available: account.balance,
            requested: amount,
        });
    }

    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;
    let cipher = account_cipher_balance(account)?;

    // Parse recipient public key — full uncompressed point (x || y, 128 hex chars).
    // Note: we parse manually instead of using parse_public_key_hex because the
    // published version has a bug where it strips a leading "04" thinking it's an
    // SEC1 uncompressed prefix, but Stark curve x-coords can naturally start with 04.
    let (rx, ry) = parse_pubkey_hex(recipient_pub_key_hex)
        .map_err(|e| WalletError::Kms(format!("invalid recipient key: {e}")))?;
    let recipient_pk = krusty_kms_crypto::StarkCurve::affine_to_projective(
        &starknet_types_core::curve::AffinePoint::new(rx, ry)
            .map_err(|e| WalletError::Kms(format!("recipient key not on curve: {e:?}")))?,
    );

    let bit_size = rpc
        .contract()
        .get_bit_size()
        .await
        .map_err(|e| WalletError::Rpc(e.to_string()))? as usize;

    let params = krusty_kms_sdk::operations::TransferParams {
        recipient_public_key: recipient_pk.clone(),
        amount,
        nonce: account.nonce,
        chain_id: chain_id_felt(&config.chain_id),
        tongo_address: tongo_addr,
        sender_address: account.starknet_address,
        current_balance: cipher,
        bit_size,
        auditor_pub_key: account.auditor_key.clone(),
    };

    let proof = krusty_kms_sdk::transfer(&account.tongo_account, params)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    let (ht_ct, ht_nonce) = match &proof.audit_transfer {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => {
            // No auditor configured: generate hint using sender-recipient ECDH
            // so both parties can decrypt the transfer amount.
            let priv_key = account.tongo_account.keypair.private_key.expose_secret();
            let shared = krusty_kms_sdk::crypto::derive_shared_secret(priv_key, &recipient_pk)
                .map_err(|e| WalletError::Kms(e.to_string()))?;
            krusty_kms_sdk::crypto::encrypt_audit_hint(amount, &shared)
                .map_err(|e| WalletError::Kms(e.to_string()))?
        }
    };
    let (hl_ct, hl_nonce) = match &proof.audit_balance {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => zero_hint(),
    };

    let from_pk = &account.tongo_account.keypair.public_key;
    let transfer_call = krusty_kms_client::build_transfer_call(
        tongo_addr,
        from_pk,
        &recipient_pk,
        &proof,
        &ht_ct,
        &ht_nonce,
        &hl_ct,
        &hl_nonce,
    )
    .map_err(|e| WalletError::Kms(e.to_string()))?;

    submitter
        .ensure_deployed(account, config, rpc)
        .await?;
    let tx_hash = submitter.submit(account, vec![transfer_call]).await?;

    // Optimistic update
    account.balance = account.balance.saturating_sub(amount);
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

// ── Withdraw ─────────────────────────────────────────────────

pub async fn execute_withdraw(
    account: &mut ActiveAccount,
    amount_sats: &str,
    recipient_address: &str,
    config: &NetworkConfig,
    rpc: &RpcClient,
    submitter: &dyn TransactionSubmitter,
) -> Result<String, WalletError> {
    let amount = sats_to_tongo_units(amount_sats)? as u128;

    if account.balance < amount {
        return Err(WalletError::InsufficientBalance {
            available: account.balance,
            requested: amount,
        });
    }

    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;
    let recipient = Felt::from_hex(recipient_address)
        .map_err(|e| WalletError::Kms(format!("invalid recipient address: {e}")))?;
    let cipher = account_cipher_balance(account)?;

    let bit_size = rpc
        .contract()
        .get_bit_size()
        .await
        .map_err(|e| WalletError::Rpc(e.to_string()))? as usize;

    let params = krusty_kms_sdk::operations::WithdrawParams {
        recipient_address: recipient,
        amount,
        nonce: account.nonce,
        chain_id: chain_id_felt(&config.chain_id),
        tongo_address: tongo_addr,
        sender_address: account.starknet_address,
        current_balance: cipher,
        bit_size,
        auditor_key: account.auditor_key.clone(),
    };

    let proof = krusty_kms_sdk::withdraw(&account.tongo_account, params)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    let (hint_ct, hint_nonce) = match &proof.audit {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => zero_hint(),
    };

    let withdraw_call =
        krusty_kms_client::build_withdraw_call(tongo_addr, &proof, &hint_ct, &hint_nonce)
            .map_err(|e| WalletError::Kms(e.to_string()))?;

    submitter
        .ensure_deployed(account, config, rpc)
        .await?;
    let tx_hash = submitter.submit(account, vec![withdraw_call]).await?;

    // Optimistic update
    account.balance = account.balance.saturating_sub(amount);
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

// ── Ragequit ─────────────────────────────────────────────────

pub async fn execute_ragequit(
    account: &mut ActiveAccount,
    recipient_address: &str,
    config: &NetworkConfig,
    _rpc: &RpcClient,
    submitter: &dyn TransactionSubmitter,
) -> Result<String, WalletError> {
    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;
    let recipient = Felt::from_hex(recipient_address)
        .map_err(|e| WalletError::Kms(format!("invalid recipient address: {e}")))?;
    let cipher = account_cipher_balance(account)?;

    let params = krusty_kms_sdk::operations::RagequitParams {
        recipient_address: recipient,
        nonce: account.nonce,
        chain_id: chain_id_felt(&config.chain_id),
        tongo_address: tongo_addr,
        sender_address: account.starknet_address,
        current_balance: cipher,
        auditor_key: account.auditor_key.clone(),
    };

    let proof = krusty_kms_sdk::ragequit(&account.tongo_account, params)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    let (hint_ct, hint_nonce) = match &proof.audit {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => zero_hint(),
    };

    let ragequit_call =
        krusty_kms_client::build_ragequit_call(tongo_addr, &proof, &hint_ct, &hint_nonce)
            .map_err(|e| WalletError::Kms(e.to_string()))?;

    submitter
        .ensure_deployed(account, config, _rpc)
        .await?;
    let tx_hash = submitter.submit(account, vec![ragequit_call]).await?;

    // Optimistic update: balance → 0
    account.balance = 0;
    account.pending = 0;
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

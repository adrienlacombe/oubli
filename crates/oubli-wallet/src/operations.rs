use starknet_types_core::curve::{AffinePoint, ProjectivePoint};
use starknet_types_core::felt::Felt;

use crate::config::NetworkConfig;
use crate::core::ActiveAccount;
use crate::denomination::{calculate_fee_sats, sats_to_tongo_units};
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

const WITHDRAW_CAIRO_STRING: Felt = Felt::from_hex_unchecked("0x7769746864726177");

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

fn subtract_point(
    lhs: &ProjectivePoint,
    rhs: &ProjectivePoint,
) -> Result<ProjectivePoint, WalletError> {
    let rhs_affine = krusty_kms_crypto::StarkCurve::projective_to_affine(rhs)
        .map_err(|e| WalletError::Kms(format!("point conversion failed: {e:?}")))?;
    let neg_rhs = krusty_kms_crypto::StarkCurve::affine_to_projective(
        &AffinePoint::new(rhs_affine.x(), -rhs_affine.y())
            .map_err(|e| WalletError::Kms(format!("invalid affine point: {e:?}")))?,
    );
    Ok(krusty_kms_crypto::StarkCurve::add(lhs, &neg_rhs))
}

/// Withdraw stores the remaining balance as current_cipher - withdraw_cipher where
/// withdraw_cipher uses deterministic randomness derived from "withdraw".
fn post_withdraw_cipher(
    current_balance: &krusty_kms_common::ElGamalCiphertext,
    owner_public_key: &ProjectivePoint,
    amount: u128,
) -> Result<krusty_kms_common::ElGamalCiphertext, WalletError> {
    let g = krusty_kms_crypto::StarkCurve::generator();
    let withdraw_l = {
        let g_amount = krusty_kms_crypto::StarkCurve::mul(&Felt::from(amount), Some(&g));
        let y_r =
            krusty_kms_crypto::StarkCurve::mul(&WITHDRAW_CAIRO_STRING, Some(owner_public_key));
        krusty_kms_crypto::StarkCurve::add(&g_amount, &y_r)
    };
    let withdraw_r = krusty_kms_crypto::StarkCurve::mul(&WITHDRAW_CAIRO_STRING, Some(&g));

    Ok(krusty_kms_common::ElGamalCiphertext {
        l: subtract_point(&current_balance.l, &withdraw_l)?,
        r: subtract_point(&current_balance.r, &withdraw_r)?,
    })
}

fn post_operation_tongo_account(
    account: &ActiveAccount,
    balance: u128,
    nonce: Felt,
) -> krusty_kms_sdk::TongoAccount {
    let mut tongo_account = account.tongo_account.clone();
    let nonce_bytes = nonce.to_bytes_be();
    let nonce_u64 = u64::from_be_bytes(nonce_bytes[24..32].try_into().unwrap_or([0u8; 8]));
    tongo_account.update_state(krusty_kms_common::AccountState {
        balance,
        pending_balance: account.pending,
        nonce: nonce_u64,
    });
    tongo_account
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

    let proof = krusty_kms_sdk::fund(&account.tongo_account, params)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

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

    let (approve_call, fund_call) = krusty_kms_client::build_fund_calls(
        tongo_addr,
        erc20_addr,
        rate,
        &proof,
        &hint_ct,
        &hint_nonce,
    )
    .map_err(|e| WalletError::Kms(e.to_string()))?;

    submitter.ensure_deployed(account, config, rpc).await?;
    let tx_hash = submitter
        .submit(account, vec![approve_call, fund_call])
        .await?;

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

    submitter.ensure_deployed(account, config, _rpc).await?;
    let tx_hash = submitter.submit(account, vec![rollover_call]).await?;

    // Optimistic update: pending → balance
    account.balance += account.pending;
    account.pending = 0;
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

// ── Fee transfer helper ──────────────────────────────────────

/// Build a fee transfer call using the updated cipher balance from a preceding operation.
/// Returns None if no fee is configured or fee is zero.
fn build_fee_call(
    account: &ActiveAccount,
    proof_account: &krusty_kms_sdk::TongoAccount,
    fee_amount: u128,
    new_cipher: krusty_kms_common::ElGamalCiphertext,
    nonce: Felt,
    bit_size: usize,
    config: &NetworkConfig,
) -> Result<Option<krusty_kms_client::starknet_rust::core::types::Call>, WalletError> {
    let collector_hex = match &config.fee_collector_pubkey {
        Some(pk) if fee_amount > 0 => pk,
        _ => return Ok(None),
    };

    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;

    let (cx, cy) = parse_pubkey_hex(collector_hex)
        .map_err(|e| WalletError::Kms(format!("invalid fee collector key: {e}")))?;
    let collector_pk = krusty_kms_crypto::StarkCurve::affine_to_projective(
        &starknet_types_core::curve::AffinePoint::new(cx, cy)
            .map_err(|e| WalletError::Kms(format!("fee collector key not on curve: {e:?}")))?,
    );

    let params = krusty_kms_sdk::operations::TransferParams {
        recipient_public_key: collector_pk.clone(),
        amount: fee_amount,
        nonce,
        chain_id: chain_id_felt(&config.chain_id),
        tongo_address: tongo_addr,
        sender_address: account.starknet_address,
        current_balance: new_cipher,
        bit_size,
        auditor_pub_key: account.auditor_key.clone(),
    };

    let proof = krusty_kms_sdk::transfer(proof_account, params)
        .map_err(|e| WalletError::Kms(e.to_string()))?;

    let (ht_ct, ht_nonce) = match &proof.audit_transfer {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => {
            let priv_key = proof_account.keypair.private_key.expose_secret();
            let shared = krusty_kms_sdk::crypto::derive_shared_secret(priv_key, &collector_pk)
                .map_err(|e| WalletError::Kms(e.to_string()))?;
            krusty_kms_sdk::crypto::encrypt_audit_hint(fee_amount, &shared)
                .map_err(|e| WalletError::Kms(e.to_string()))?
        }
    };
    let (hl_ct, hl_nonce) = match &proof.audit_balance {
        Some(audit) => (audit.hint_ciphertext, audit.hint_nonce),
        None => zero_hint(),
    };

    let from_pk = &proof_account.keypair.public_key;
    let call = krusty_kms_client::build_transfer_call(
        tongo_addr,
        from_pk,
        &collector_pk,
        &proof,
        &ht_ct,
        &ht_nonce,
        &hl_ct,
        &hl_nonce,
    )
    .map_err(|e| WalletError::Kms(e.to_string()))?;

    Ok(Some(call))
}

fn fee_enabled(config: &NetworkConfig) -> bool {
    config.fee_collector_pubkey.is_some() && config.fee_percent > 0.0
}

/// Compute fee in tongo units from an amount in sats and config.
/// Returns 0 if fee is not configured.
fn compute_fee_units(amount_sats: &str, config: &NetworkConfig) -> u64 {
    if !fee_enabled(config) {
        return 0;
    }
    let sats: u64 = match amount_sats.trim().parse() {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let fee_sats = calculate_fee_sats(sats, config.fee_percent);
    fee_sats / 10 // RATE = 10
}

fn compute_withdraw_fee_units(
    amount_sats: &str,
    sender_address: &Felt,
    recipient_address: &Felt,
    config: &NetworkConfig,
) -> u64 {
    if sender_address == recipient_address {
        return 0;
    }

    compute_fee_units(amount_sats, config)
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

    submitter.ensure_deployed(account, config, rpc).await?;
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
    let tongo_addr =
        Felt::from_hex(&config.tongo_contract).map_err(|e| WalletError::Kms(e.to_string()))?;
    let recipient = Felt::from_hex(recipient_address)
        .map_err(|e| WalletError::Kms(format!("invalid recipient address: {e}")))?;
    let fee_units =
        compute_withdraw_fee_units(amount_sats, &account.starknet_address, &recipient, config)
            as u128;

    if account.balance < amount + fee_units {
        return Err(WalletError::InsufficientBalance {
            available: account.balance,
            requested: amount + fee_units,
        });
    }

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
        current_balance: cipher.clone(),
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

    // Build fee transfer call using the actual post-withdraw stored balance cipher.
    let fee_call = if fee_units > 0 {
        let post_withdraw_balance = account.balance.saturating_sub(amount);
        let fee_nonce = account.nonce + Felt::ONE;
        let fee_account = post_operation_tongo_account(account, post_withdraw_balance, fee_nonce);
        let post_withdraw_cipher =
            post_withdraw_cipher(&cipher, &account.tongo_account.keypair.public_key, amount)?;
        build_fee_call(
            account,
            &fee_account,
            fee_units,
            post_withdraw_cipher,
            fee_nonce,
            bit_size,
            config,
        )?
    } else {
        None
    };

    submitter.ensure_deployed(account, config, rpc).await?;

    let mut calls = vec![withdraw_call];
    if let Some(fc) = fee_call {
        calls.push(fc);
    }
    let tx_hash = submitter.submit(account, calls).await?;

    // Optimistic update
    account.balance = account.balance.saturating_sub(amount + fee_units);
    account.nonce = if fee_units > 0 {
        account.nonce + Felt::TWO
    } else {
        account.nonce + Felt::ONE
    };

    Ok(tx_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pubkey_hex_parses_full_coordinates() {
        let input = format!("0x{:064x}{:064x}", 0x123_u64, 0x456_u64);
        let (x, y) = parse_pubkey_hex(&input).unwrap();

        assert_eq!(x, Felt::from(0x123_u64));
        assert_eq!(y, Felt::from(0x456_u64));
    }

    #[test]
    fn parse_pubkey_hex_ignores_whitespace_and_missing_prefix() {
        let input = format!("  \n{:064x}{:064x}\n", 0xabc_u64, 0xdef_u64);
        let (x, y) = parse_pubkey_hex(&input).unwrap();

        assert_eq!(x, Felt::from(0xabc_u64));
        assert_eq!(y, Felt::from(0xdef_u64));
    }

    #[test]
    fn parse_pubkey_hex_rejects_non_zero_leading_overflow() {
        let input = format!("1{}", "0".repeat(128));
        let err = parse_pubkey_hex(&input).unwrap_err();
        assert!(err.contains("too long"));
    }

    #[test]
    fn parse_pubkey_hex_rejects_too_short_values() {
        let err = parse_pubkey_hex("0").unwrap_err();
        assert!(err.contains("unexpected length"));
    }

    #[test]
    fn self_withdraw_does_not_charge_fee() {
        let mut config = crate::networks::sepolia::config();
        config.fee_percent = 1.0;
        config.fee_collector_pubkey = Some("collector".into());

        let sender = Felt::from_hex("0x123").unwrap();
        let recipient = Felt::from_hex("0x123").unwrap();

        assert_eq!(
            compute_withdraw_fee_units("1000", &sender, &recipient, &config),
            0
        );
    }

    #[test]
    fn external_withdraw_charges_configured_fee() {
        let mut config = crate::networks::sepolia::config();
        config.fee_percent = 1.0;
        config.fee_collector_pubkey = Some("collector".into());

        let sender = Felt::from_hex("0x123").unwrap();
        let recipient = Felt::from_hex("0x456").unwrap();

        assert_eq!(
            compute_withdraw_fee_units("1000", &sender, &recipient, &config),
            1
        );
    }

    #[test]
    fn fee_units_return_zero_when_fee_disabled_or_amount_invalid() {
        let mut config = crate::networks::sepolia::config();
        config.fee_percent = 1.0;

        assert_eq!(compute_fee_units("1000", &config), 0);

        config.fee_collector_pubkey = Some("collector".into());
        assert_eq!(compute_fee_units("not-a-number", &config), 0);
    }
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

    submitter.ensure_deployed(account, config, _rpc).await?;
    let tx_hash = submitter.submit(account, vec![ragequit_call]).await?;

    // Optimistic update: balance → 0
    account.balance = 0;
    account.pending = 0;
    account.nonce = account.nonce + Felt::ONE;

    Ok(tx_hash)
}

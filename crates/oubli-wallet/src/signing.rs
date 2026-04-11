use starknet_types_core::felt::Felt;

use crate::error::WalletError;

// Re-export the starknet-rs TypedData for use in submitter.rs
pub use krusty_kms_client::starknet_rust::core::types::typed_data::TypedData;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalCall {
    to: Felt,
    selector: Felt,
    calldata: Vec<Felt>,
}

// ── SNIP-12 message hash ─────────────────────────────────────

/// Compute SNIP-12 message hash using starknet-rs's well-tested TypedData implementation.
/// Supports both revision 0 (Pedersen) and revision 1 (Poseidon), including enums,
/// merkle trees, u256, preset types, etc.
pub fn compute_message_hash(typed_data: &TypedData, account: &Felt) -> Result<Felt, WalletError> {
    // Convert starknet-types-core Felt to starknet-rs Felt
    let account_rs =
        krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(&account.to_bytes_be());

    let hash_rs = typed_data
        .message_hash(account_rs)
        .map_err(|e| WalletError::Signing(format!("SNIP-12 message hash failed: {e}")))?;

    // Convert back to starknet-types-core Felt
    Ok(Felt::from_bytes_be(&hash_rs.to_bytes_be()))
}

/// Compute SNIP-12 message hash for OutsideExecution typed data, handling the `selector`
/// type correctly.
///
/// starknet-rs always applies `starknet_keccak` to `selector` values, even hex strings.
/// But starknet.js and on-chain Argent contracts treat hex selector values as raw felts.
/// This function manually computes the message hash using the correct encoding:
/// - Domain hash and type hashes from starknet-rs (correct)
/// - Message body encoded manually with `selector` values as raw felts
pub fn compute_outside_execution_hash(
    typed_data_json: &serde_json::Value,
    account: &Felt,
) -> Result<Felt, WalletError> {
    use krusty_kms_crypto::hash::poseidon_hash_many;

    // Parse TypedData to get domain hash and type hashes (both correct in starknet-rs)
    let typed_data: TypedData = serde_json::from_value(typed_data_json.clone())
        .map_err(|e| WalletError::Signing(format!("parse typed data: {e}")))?;

    let domain_hash = rs_to_native(typed_data.encoder().domain().encoded_hash());

    let call_type_hash = rs_to_native(
        typed_data
            .encoder()
            .types()
            .get_type_hash("Call")
            .map_err(|e| WalletError::Signing(format!("Call type hash: {e}")))?,
    );
    let oe_type_hash = rs_to_native(
        typed_data
            .encoder()
            .types()
            .get_type_hash("OutsideExecution")
            .map_err(|e| WalletError::Signing(format!("OutsideExecution type hash: {e}")))?,
    );

    // "StarkNet Message" as Cairo short string
    let prefix = Felt::from_hex("0x537461726b4e6574204d657373616765")
        .map_err(|e| WalletError::Signing(format!("prefix: {e}")))?;

    // Parse message from the original JSON
    let message = typed_data_json
        .get("message")
        .ok_or_else(|| WalletError::Signing("no message in typed data".into()))?;

    // Encode each Call with selector as RAW FELT (not keccak'd)
    let calls = canonical_calls_from_message(message, WalletError::Signing)?;

    let mut call_hashes = Vec::with_capacity(calls.len());
    for call in &calls {
        let cd_hash = poseidon_hash_many(&call.calldata);
        call_hashes.push(poseidon_hash_many(&[
            call_type_hash,
            call.to,
            call.selector,
            cd_hash,
        ]));
    }
    let calls_hash = poseidon_hash_many(&call_hashes);

    // Encode OutsideExecution
    let caller = json_felt(message.get("Caller"), "Caller")?;
    let nonce = json_felt(message.get("Nonce"), "Nonce")?;
    let execute_after = json_u128(message.get("Execute After"), "Execute After")?;
    let execute_before = json_u128(message.get("Execute Before"), "Execute Before")?;

    let oe_hash = poseidon_hash_many(&[
        oe_type_hash,
        caller,
        nonce,
        execute_after,
        execute_before,
        calls_hash,
    ]);

    // Final SNIP-12 message hash
    Ok(poseidon_hash_many(&[
        prefix,
        domain_hash,
        *account,
        oe_hash,
    ]))
}

pub fn sign_validated_paymaster_invoke(
    typed_data_json: &serde_json::Value,
    expected_calls: &[serde_json::Value],
    account: &Felt,
    chain_id: &str,
    private_key: &Felt,
) -> Result<(Felt, Felt), WalletError> {
    validate_paymaster_invoke_typed_data(typed_data_json, expected_calls, chain_id)?;
    let hash = compute_outside_execution_hash(typed_data_json, account)?;
    sign_message_hash(&hash, private_key)
}

/// Convert starknet-rs Felt to starknet-types-core Felt.
fn rs_to_native(f: krusty_kms_client::starknet_rust::core::types::Felt) -> Felt {
    Felt::from_bytes_be(&f.to_bytes_be())
}

/// Parse a JSON string value as a raw Felt (hex or decimal).
fn json_felt(v: Option<&serde_json::Value>, ctx: &str) -> Result<Felt, WalletError> {
    let s = v
        .and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::Signing(format!("expected string for {ctx}")))?;
    Felt::from_hex(s).or_else(|_| {
        Felt::from_dec_str(s)
            .map_err(|e| WalletError::Signing(format!("parse felt {ctx} '{s}': {e}")))
    })
}

/// Parse a JSON value as u128 → Felt (handles both hex string and integer).
fn json_u128(v: Option<&serde_json::Value>, ctx: &str) -> Result<Felt, WalletError> {
    match v {
        Some(serde_json::Value::Number(n)) => {
            let val = n
                .as_u64()
                .ok_or_else(|| WalletError::Signing(format!("{ctx}: not a u64")))?;
            Ok(Felt::from(val))
        }
        Some(serde_json::Value::String(s)) => {
            let val = if let Some(hex) = s.strip_prefix("0x") {
                u128::from_str_radix(hex, 16)
            } else {
                s.parse::<u128>()
            }
            .map_err(|e| WalletError::Signing(format!("{ctx}: {e}")))?;
            Ok(Felt::from(val))
        }
        _ => Err(WalletError::Signing(format!(
            "expected number or string for {ctx}"
        ))),
    }
}

/// Parse typed data JSON into TypedData.
pub fn parse_typed_data(json: &serde_json::Value) -> Result<TypedData, WalletError> {
    serde_json::from_value(json.clone())
        .map_err(|e| WalletError::Signing(format!("parse typed data: {e}")))
}

/// Sign a message hash with starknet private key (ECDSA on Stark curve).
/// Returns (r, s) as Felt values.
pub fn sign_message_hash(hash: &Felt, private_key: &Felt) -> Result<(Felt, Felt), WalletError> {
    // Convert to starknet-rs Felt for signing
    let sk_rs = krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(
        &private_key.to_bytes_be(),
    );
    let hash_rs =
        krusty_kms_client::starknet_rust::core::types::Felt::from_bytes_be(&hash.to_bytes_be());

    let signing_key =
        krusty_kms_client::starknet_rust::signers::SigningKey::from_secret_scalar(sk_rs);
    let signature = signing_key
        .sign(&hash_rs)
        .map_err(|e| WalletError::Signing(format!("ECDSA sign failed: {e}")))?;

    // Convert back to starknet-types-core Felt
    let r = Felt::from_bytes_be(&signature.r.to_bytes_be());
    let s = Felt::from_bytes_be(&signature.s.to_bytes_be());

    Ok((r, s))
}

/// Validate that TypedData calls match the expected calls (prevent paymaster injection).
pub fn validate_typed_data_calls(
    typed_data_json: &serde_json::Value,
    expected_calls: &[serde_json::Value],
) -> Result<(), WalletError> {
    let message = typed_data_json
        .get("message")
        .ok_or_else(|| WalletError::TypedDataValidation("no message field in typed data".into()))?;
    let actual_calls = canonical_calls_from_message(message, WalletError::TypedDataValidation)?;
    let expected_calls = canonical_calls_from_expected(expected_calls)?;

    validate_canonical_calls(&actual_calls, &expected_calls)
}

pub fn validate_paymaster_invoke_typed_data(
    typed_data_json: &serde_json::Value,
    expected_calls: &[serde_json::Value],
    expected_chain_id: &str,
) -> Result<(), WalletError> {
    let primary_type = typed_data_json
        .get("primaryType")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            WalletError::TypedDataValidation("missing primaryType in typed data".into())
        })?;
    if primary_type != "OutsideExecution" {
        return Err(WalletError::TypedDataValidation(format!(
            "unexpected primaryType: {primary_type}"
        )));
    }

    let domain = typed_data_json
        .get("domain")
        .ok_or_else(|| WalletError::TypedDataValidation("missing domain in typed data".into()))?;
    let actual_chain_id = parse_chain_id_field(
        domain,
        &["chainId", "chain_id"],
        "domain.chainId",
        WalletError::TypedDataValidation,
    )?;
    let expected_chain_id =
        short_string_to_felt(expected_chain_id).map_err(WalletError::TypedDataValidation)?;
    if actual_chain_id != expected_chain_id {
        return Err(WalletError::TypedDataValidation(format!(
            "chain ID mismatch: expected {expected_chain_id}, got {actual_chain_id}"
        )));
    }

    validate_typed_data_calls(typed_data_json, expected_calls)
}

fn canonical_calls_from_message(
    message: &serde_json::Value,
    mk_err: fn(String) -> WalletError,
) -> Result<Vec<CanonicalCall>, WalletError> {
    let calls = message
        .get("Calls")
        .or_else(|| message.get("calls"))
        .and_then(|value| value.as_array())
        .ok_or_else(|| mk_err("no calls field in typed data message".into()))?;
    calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
            canonical_call_from_value(call, &format!("message.calls[{index}]"), mk_err)
        })
        .collect()
}

fn canonical_calls_from_expected(
    expected_calls: &[serde_json::Value],
) -> Result<Vec<CanonicalCall>, WalletError> {
    expected_calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
            canonical_call_from_value(
                call,
                &format!("expected_calls[{index}]"),
                WalletError::TypedDataValidation,
            )
        })
        .collect()
}

fn canonical_call_from_value(
    call: &serde_json::Value,
    ctx: &str,
    mk_err: fn(String) -> WalletError,
) -> Result<CanonicalCall, WalletError> {
    let to = parse_felt_field(call, &["To", "to"], &format!("{ctx}.to"), mk_err)?;
    let selector = parse_felt_field(
        call,
        &["Selector", "selector"],
        &format!("{ctx}.selector"),
        mk_err,
    )?;
    let calldata = parse_felt_array_field(
        call,
        &["Calldata", "calldata"],
        &format!("{ctx}.calldata"),
        mk_err,
    )?;

    Ok(CanonicalCall {
        to,
        selector,
        calldata,
    })
}

fn validate_canonical_calls(
    actual_calls: &[CanonicalCall],
    expected_calls: &[CanonicalCall],
) -> Result<(), WalletError> {
    if actual_calls.len() != expected_calls.len() {
        return Err(WalletError::TypedDataValidation(format!(
            "call count mismatch: expected {}, got {}",
            expected_calls.len(),
            actual_calls.len()
        )));
    }

    for (index, (actual, expected)) in actual_calls.iter().zip(expected_calls.iter()).enumerate() {
        if actual.to != expected.to {
            return Err(WalletError::TypedDataValidation(format!(
                "call[{index}] to mismatch"
            )));
        }
        if actual.selector != expected.selector {
            return Err(WalletError::TypedDataValidation(format!(
                "call[{index}] selector mismatch"
            )));
        }
        if actual.calldata != expected.calldata {
            return Err(WalletError::TypedDataValidation(format!(
                "call[{index}] calldata mismatch"
            )));
        }
    }

    Ok(())
}

fn parse_felt_array_field(
    object: &serde_json::Value,
    keys: &[&str],
    ctx: &str,
    mk_err: fn(String) -> WalletError,
) -> Result<Vec<Felt>, WalletError> {
    let values = get_object_field(object, keys)
        .and_then(|value| value.as_array())
        .ok_or_else(|| mk_err(format!("missing {ctx}")))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| parse_felt_value(value, &format!("{ctx}[{index}]"), mk_err))
        .collect()
}

fn parse_felt_field(
    object: &serde_json::Value,
    keys: &[&str],
    ctx: &str,
    mk_err: fn(String) -> WalletError,
) -> Result<Felt, WalletError> {
    let value = get_object_field(object, keys).ok_or_else(|| mk_err(format!("missing {ctx}")))?;
    parse_felt_value(value, ctx, mk_err)
}

fn parse_chain_id_field(
    object: &serde_json::Value,
    keys: &[&str],
    ctx: &str,
    mk_err: fn(String) -> WalletError,
) -> Result<Felt, WalletError> {
    let value = get_object_field(object, keys).ok_or_else(|| mk_err(format!("missing {ctx}")))?;
    match value {
        serde_json::Value::String(s) => parse_chain_id_value(s, ctx, mk_err),
        serde_json::Value::Number(_) => parse_felt_value(value, ctx, mk_err),
        _ => Err(mk_err(format!("expected string or number for {ctx}"))),
    }
}

fn parse_chain_id_value(
    value: &str,
    ctx: &str,
    mk_err: fn(String) -> WalletError,
) -> Result<Felt, WalletError> {
    if let Ok(felt) = Felt::from_hex(value) {
        return Ok(felt);
    }
    if let Ok(felt) = Felt::from_dec_str(value) {
        return Ok(felt);
    }
    short_string_to_felt(value).map_err(|e| mk_err(format!("invalid {ctx}: {e}")))
}

fn parse_felt_value(
    value: &serde_json::Value,
    ctx: &str,
    mk_err: fn(String) -> WalletError,
) -> Result<Felt, WalletError> {
    match value {
        serde_json::Value::String(s) => Felt::from_hex(s)
            .or_else(|_| Felt::from_dec_str(s))
            .map_err(|e| mk_err(format!("parse felt {ctx} '{s}': {e}"))),
        serde_json::Value::Number(n) => {
            let felt_value = n
                .as_u64()
                .ok_or_else(|| mk_err(format!("{ctx}: not a u64")))?;
            Ok(Felt::from(felt_value))
        }
        _ => Err(mk_err(format!("expected string or number for {ctx}"))),
    }
}

fn get_object_field<'a>(
    object: &'a serde_json::Value,
    keys: &[&str],
) -> Option<&'a serde_json::Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn short_string_to_felt(value: &str) -> Result<Felt, String> {
    if value.len() > 31 {
        return Err(format!("short string too long: {}", value.len()));
    }
    let bytes = value.as_bytes();
    let mut padded = [0u8; 32];
    padded[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(Felt::from_bytes_be(&padded))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paymaster_typed_data(calls: serde_json::Value, chain_id: &str) -> serde_json::Value {
        serde_json::json!({
            "primaryType": "OutsideExecution",
            "domain": {
                "chainId": chain_id,
            },
            "message": {
                "Calls": calls,
            }
        })
    }

    #[test]
    fn test_sign_and_verify() {
        // Use a deterministic test key
        let private_key =
            Felt::from_hex("0x0139fe4d6f02e666e86a6f58e65060f115cd3c185bd9e98bd829636931458f79")
                .unwrap();
        let message_hash =
            Felt::from_hex("0x06fea80189363a786037ed3e7ba546dad0ef7de49fccae0e31eb658b7dd4ea76")
                .unwrap();

        let (r, s) = sign_message_hash(&message_hash, &private_key).unwrap();
        assert_ne!(r, Felt::ZERO);
        assert_ne!(s, Felt::ZERO);
    }

    #[test]
    fn test_call_validation_rejects_mismatch() {
        let typed_data_json = serde_json::json!({
            "message": {
                "calls": [
                    {"To": "0x1", "Selector": "0x2", "Calldata": []},
                    {"To": "0x3", "Selector": "0x4", "Calldata": []}
                ]
            }
        });

        // Expected has only 1 call — mismatch
        let expected = vec![serde_json::json!({"To": "0x1", "Selector": "0x2", "Calldata": []})];

        let result = validate_typed_data_calls(&typed_data_json, &expected);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("call count mismatch"));
    }

    #[test]
    fn test_call_validation_accepts_match() {
        let calls = vec![
            serde_json::json!({"To": "0x1", "Selector": "0x2", "Calldata": ["0xa"]}),
            serde_json::json!({"To": "0x3", "Selector": "0x4", "Calldata": ["0xb"]}),
        ];

        let typed_data_json = serde_json::json!({
            "message": {
                "Calls": [
                    {"To": "0x1", "Selector": "0x2", "Calldata": ["0xa"]},
                    {"To": "0x3", "Selector": "0x4", "Calldata": ["0xb"]},
                ]
            }
        });

        assert!(validate_typed_data_calls(&typed_data_json, &calls).is_ok());
    }

    #[test]
    fn test_call_validation_rejects_calldata_mismatch() {
        let expected = vec![serde_json::json!({
            "to": "0x1",
            "selector": "0x2",
            "calldata": ["0xa", "0xb"],
        })];
        let typed_data_json = serde_json::json!({
            "message": {
                "Calls": [
                    {"To": "0x1", "Selector": "0x2", "Calldata": ["0xa", "0xc"]},
                ]
            }
        });

        let err = validate_typed_data_calls(&typed_data_json, &expected).unwrap_err();
        assert!(err.to_string().contains("calldata mismatch"));
    }

    #[test]
    fn test_paymaster_validation_rejects_wrong_chain_id() {
        let calls = serde_json::json!([
            {"To": "0x1", "Selector": "0x2", "Calldata": []}
        ]);
        let typed_data_json = paymaster_typed_data(calls, "SN_MAIN");
        let expected = vec![serde_json::json!({
            "to": "0x1",
            "selector": "0x2",
            "calldata": [],
        })];

        let err = validate_paymaster_invoke_typed_data(&typed_data_json, &expected, "SN_SEPOLIA")
            .unwrap_err();
        assert!(err.to_string().contains("chain ID mismatch"));
    }

    #[test]
    fn test_parse_and_hash_v1_typed_data() {
        let raw = r###"{
  "types": {
    "StarknetDomain": [
      { "name": "name", "type": "shortstring" },
      { "name": "version", "type": "shortstring" },
      { "name": "chainId", "type": "shortstring" },
      { "name": "revision", "type": "shortstring" }
    ],
    "Example Message": [
      { "name": "Name", "type": "string" },
      { "name": "Some Array", "type": "u128*" },
      { "name": "Some Object", "type": "My Object" }
    ],
    "My Object": [
      { "name": "Some Selector", "type": "selector" },
      { "name": "Some Contract Address", "type": "ContractAddress" }
    ]
  },
  "primaryType": "Example Message",
  "domain": {
    "name": "Starknet Example",
    "version": "1",
    "chainId": "SN_MAIN",
    "revision": "1"
  },
  "message": {
    "Name": "some name",
    "Some Array": [1, 2, 3, 4],
    "Some Object": {
      "Some Selector": "transfer",
      "Some Contract Address": "0x0123"
    }
  }
}"###;

        let json: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed_data = parse_typed_data(&json).unwrap();
        let account = Felt::from_hex("0x1234").unwrap();
        let hash = compute_message_hash(&typed_data, &account).unwrap();

        // Known-good value from starknet-rs test suite
        assert_eq!(
            hash,
            Felt::from_hex("0x045bca39274d2b7fdf7dc7c4ecf75f6549f614ce44359cc62ec106f4e5cc87b4")
                .unwrap()
        );
    }
}

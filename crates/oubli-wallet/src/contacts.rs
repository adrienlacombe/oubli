use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use oubli_store::PlatformStorage;

use crate::error::WalletError;

use starknet_types_core::felt::Felt;

const CONTACTS_KEY: &str = "oubli.contacts";
const MAX_CONTACTS: usize = 500;

// ── Types ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressType {
    /// Tongo public key (128 hex chars) for private Oubli transfers.
    Oubli,
    /// Starknet/L1 address.
    Starknet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactAddress {
    pub address: String,
    pub address_type: AddressType,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub addresses: Vec<ContactAddress>,
    pub notes: Option<String>,
    pub created_at: u64,
    pub last_used_at: u64,
}

// ── ID generation ────────────────────────────────────────────

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:032x}")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Storage helpers ──────────────────────────────────────────

// ── Validation ───────────────────────────────────────────────

fn validate_address(addr: &ContactAddress) -> Result<(), WalletError> {
    let trimmed = addr.address.trim();
    if trimmed.is_empty() {
        return Err(WalletError::Kms("address cannot be empty".into()));
    }
    let stripped = trimmed.strip_prefix("0x").unwrap_or(trimmed);

    // Must be valid hex
    if !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WalletError::Kms(format!(
            "address contains non-hex characters: {}",
            truncate(trimmed, 20)
        )));
    }

    match addr.address_type {
        AddressType::Oubli => {
            // Oubli public key: uncompressed x||y, ~128 hex chars (2-140 accepted,
            // matching parse_pubkey_hex in operations.rs).
            if stripped.len() < 2 || stripped.len() > 140 {
                return Err(WalletError::Kms(format!(
                    "Oubli address has invalid length {} (expected ~128 hex chars)",
                    stripped.len()
                )));
            }
        }
        AddressType::Starknet => {
            // Starknet address: up to 64 hex chars, must parse as a Felt.
            if stripped.len() > 64 {
                return Err(WalletError::Kms(format!(
                    "Starknet address too long ({} hex chars, max 64)",
                    stripped.len()
                )));
            }
            Felt::from_hex(trimmed)
                .map_err(|e| WalletError::Kms(format!("invalid Starknet address: {e}")))?;
        }
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ── Storage helpers ──────────────────────────────────────────

fn load_contacts(storage: &dyn PlatformStorage) -> Vec<Contact> {
    storage
        .secure_load(CONTACTS_KEY)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_contacts(storage: &dyn PlatformStorage, contacts: &[Contact]) -> Result<(), WalletError> {
    let json = serde_json::to_vec(contacts)
        .map_err(|e| WalletError::Kms(format!("contacts serialization: {e}")))?;
    storage
        .secure_store(CONTACTS_KEY, &json)
        .map_err(|e| WalletError::Kms(format!("contacts storage: {e}")))
}

// ── Public API (free functions taking storage) ───────────────

/// Return all contacts, sorted by last_used_at descending.
pub fn get_contacts(storage: &Arc<dyn PlatformStorage>) -> Vec<Contact> {
    let mut contacts = load_contacts(storage.as_ref());
    contacts.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at));
    contacts
}

/// Get a single contact by ID.
pub fn get_contact(storage: &Arc<dyn PlatformStorage>, contact_id: &str) -> Option<Contact> {
    load_contacts(storage.as_ref())
        .into_iter()
        .find(|c| c.id == contact_id)
}

/// Upsert a contact. If `contact.id` is empty, generates a new one.
/// Returns the (possibly generated) ID.
pub fn save_contact(
    storage: &Arc<dyn PlatformStorage>,
    mut contact: Contact,
) -> Result<String, WalletError> {
    if contact.name.trim().is_empty() {
        return Err(WalletError::Kms("contact name cannot be empty".into()));
    }
    if contact.addresses.is_empty() {
        return Err(WalletError::Kms(
            "contact must have at least one address".into(),
        ));
    }
    for addr in &contact.addresses {
        validate_address(addr)?;
    }

    let mut contacts = load_contacts(storage.as_ref());

    if contact.id.is_empty() {
        // New contact
        contact.id = generate_id();
        contact.created_at = now_secs();
        if contact.last_used_at == 0 {
            contact.last_used_at = contact.created_at;
        }
        if contacts.len() >= MAX_CONTACTS {
            return Err(WalletError::Kms(format!(
                "contact limit reached ({MAX_CONTACTS})"
            )));
        }
        contacts.push(contact.clone());
    } else {
        // Update existing
        if let Some(existing) = contacts.iter_mut().find(|c| c.id == contact.id) {
            existing.name = contact.name.clone();
            existing.addresses = contact.addresses.clone();
            existing.notes = contact.notes.clone();
        } else {
            return Err(WalletError::Kms(format!(
                "contact not found: {}",
                contact.id
            )));
        }
    }

    save_contacts(storage.as_ref(), &contacts)?;
    Ok(contact.id)
}

/// Delete a contact by ID.
pub fn delete_contact(
    storage: &Arc<dyn PlatformStorage>,
    contact_id: &str,
) -> Result<(), WalletError> {
    let mut contacts = load_contacts(storage.as_ref());
    let before = contacts.len();
    contacts.retain(|c| c.id != contact_id);
    if contacts.len() == before {
        return Err(WalletError::Kms(format!("contact not found: {contact_id}")));
    }
    save_contacts(storage.as_ref(), &contacts)
}

/// Find a contact that has a matching address (case-insensitive hex comparison).
pub fn find_contact_by_address(
    storage: &Arc<dyn PlatformStorage>,
    address: &str,
) -> Option<Contact> {
    let needle = address.to_lowercase();
    load_contacts(storage.as_ref()).into_iter().find(|c| {
        c.addresses
            .iter()
            .any(|a| a.address.to_lowercase() == needle)
    })
}

/// Update last_used_at timestamp for a contact.
pub fn update_contact_last_used(
    storage: &Arc<dyn PlatformStorage>,
    contact_id: &str,
) -> Result<(), WalletError> {
    let mut contacts = load_contacts(storage.as_ref());
    if let Some(c) = contacts.iter_mut().find(|c| c.id == contact_id) {
        c.last_used_at = now_secs();
        save_contacts(storage.as_ref(), &contacts)
    } else {
        Err(WalletError::Kms(format!("contact not found: {contact_id}")))
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use oubli_store::MockPlatformStorage;

    // Valid Starknet address (64 hex chars with 0x prefix)
    const STARKNET_ADDR_1: &str =
        "0x05f42d1042aa8013909f87f8fc4f9854ec174e9b94ae86b7aaf44f979c7a7a3b";
    const STARKNET_ADDR_2: &str =
        "0x01a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2";

    // Valid Oubli public key (128 hex chars — x || y coordinates)
    const OUBLI_PUBKEY_1: &str = "0102030405060708091011121314151617181920212223242526272829303132\
         3334353637383940414243444546474849505152535455565758596061626364";
    const OUBLI_PUBKEY_2: &str = "aabbccdd00112233445566778899aabbccddeeff00112233445566778899aabb\
         ccddeeff00112233445566778899aabbccddeeff00112233445566778899aabb";

    fn mock_storage() -> Arc<dyn PlatformStorage> {
        Arc::new(MockPlatformStorage::new())
    }

    fn sample_contact(name: &str, addr: &str, addr_type: AddressType) -> Contact {
        Contact {
            id: String::new(),
            name: name.to_string(),
            addresses: vec![ContactAddress {
                address: addr.to_string(),
                address_type: addr_type,
                label: None,
            }],
            notes: None,
            created_at: 0,
            last_used_at: 0,
        }
    }

    #[test]
    fn empty_on_fresh_storage() {
        let s = mock_storage();
        assert!(get_contacts(&s).is_empty());
    }

    #[test]
    fn add_and_retrieve() {
        let s = mock_storage();
        let c = sample_contact("Alice", STARKNET_ADDR_1, AddressType::Starknet);
        let id = save_contact(&s, c).unwrap();
        assert!(!id.is_empty());

        let contacts = get_contacts(&s);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Alice");
        assert_eq!(contacts[0].id, id);
    }

    #[test]
    fn update_existing() {
        let s = mock_storage();
        let c = sample_contact("Bob", OUBLI_PUBKEY_1, AddressType::Oubli);
        let id = save_contact(&s, c).unwrap();

        let mut updated = get_contact(&s, &id).unwrap();
        updated.name = "Bobby".to_string();
        save_contact(&s, updated).unwrap();

        let contacts = get_contacts(&s);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].name, "Bobby");
    }

    #[test]
    fn delete() {
        let s = mock_storage();
        let c = sample_contact("Charlie", STARKNET_ADDR_1, AddressType::Starknet);
        let id = save_contact(&s, c).unwrap();

        delete_contact(&s, &id).unwrap();
        assert!(get_contacts(&s).is_empty());
    }

    #[test]
    fn delete_nonexistent() {
        let s = mock_storage();
        assert!(delete_contact(&s, "nonexistent").is_err());
    }

    #[test]
    fn find_by_address() {
        let s = mock_storage();
        let c = sample_contact("Dave", STARKNET_ADDR_1, AddressType::Starknet);
        save_contact(&s, c).unwrap();

        let found = find_contact_by_address(&s, STARKNET_ADDR_1);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Dave");

        assert!(find_contact_by_address(&s, "0xdead").is_none());
    }

    #[test]
    fn find_by_address_multiple_addresses() {
        let s = mock_storage();
        let c = Contact {
            id: String::new(),
            name: "Eve".to_string(),
            addresses: vec![
                ContactAddress {
                    address: STARKNET_ADDR_1.to_string(),
                    address_type: AddressType::Starknet,
                    label: Some("Main".to_string()),
                },
                ContactAddress {
                    address: OUBLI_PUBKEY_1.to_string(),
                    address_type: AddressType::Oubli,
                    label: Some("Privacy".to_string()),
                },
            ],
            notes: Some("Test contact".to_string()),
            created_at: 0,
            last_used_at: 0,
        };
        save_contact(&s, c).unwrap();

        assert!(find_contact_by_address(&s, STARKNET_ADDR_1).is_some());
        assert!(find_contact_by_address(&s, OUBLI_PUBKEY_1).is_some());
    }

    #[test]
    fn update_last_used() {
        let s = mock_storage();
        let c = sample_contact("Frank", STARKNET_ADDR_1, AddressType::Starknet);
        let id = save_contact(&s, c).unwrap();

        let before = get_contact(&s, &id).unwrap().last_used_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        update_contact_last_used(&s, &id).unwrap();
        let after = get_contact(&s, &id).unwrap().last_used_at;
        assert!(after >= before);
    }

    #[test]
    fn empty_name_rejected() {
        let s = mock_storage();
        let c = sample_contact("", STARKNET_ADDR_1, AddressType::Starknet);
        assert!(save_contact(&s, c).is_err());
    }

    #[test]
    fn sorted_by_last_used() {
        let s = mock_storage();

        let c1 = sample_contact("Oldest", STARKNET_ADDR_1, AddressType::Starknet);
        let id1 = save_contact(&s, c1).unwrap();

        let c2 = sample_contact("Newest", STARKNET_ADDR_2, AddressType::Starknet);
        let _id2 = save_contact(&s, c2).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));
        update_contact_last_used(&s, &id1).unwrap();

        let contacts = get_contacts(&s);
        assert_eq!(contacts[0].name, "Oldest");
        assert_eq!(contacts[1].name, "Newest");
    }

    // ── Validation tests ─────────────────────────────────────

    #[test]
    fn rejects_non_hex_starknet() {
        let s = mock_storage();
        let c = sample_contact("Bad", "0xZZZZZZ", AddressType::Starknet);
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("non-hex"));
    }

    #[test]
    fn rejects_non_hex_oubli() {
        let s = mock_storage();
        let c = sample_contact("Bad", "not-hex-at-all!", AddressType::Oubli);
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("non-hex"));
    }

    #[test]
    fn rejects_too_long_starknet() {
        let s = mock_storage();
        // 65 hex chars after 0x — exceeds 64 limit
        let addr = format!("0x{}", "a".repeat(65));
        let c = sample_contact("Long", &addr, AddressType::Starknet);
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn rejects_too_short_oubli() {
        let s = mock_storage();
        // Single hex char — too short for a public key
        let c = sample_contact("Short", "a", AddressType::Oubli);
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("invalid length"));
    }

    #[test]
    fn rejects_too_long_oubli() {
        let s = mock_storage();
        let addr = "a".repeat(141);
        let c = sample_contact("TooLong", &addr, AddressType::Oubli);
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("invalid length"));
    }

    #[test]
    fn rejects_empty_address() {
        let s = mock_storage();
        let c = sample_contact("Empty", "", AddressType::Starknet);
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn rejects_no_addresses() {
        let s = mock_storage();
        let c = Contact {
            id: String::new(),
            name: "NoAddr".to_string(),
            addresses: vec![],
            notes: None,
            created_at: 0,
            last_used_at: 0,
        };
        let err = save_contact(&s, c).unwrap_err();
        assert!(err.to_string().contains("at least one address"));
    }

    #[test]
    fn accepts_valid_starknet_short() {
        let s = mock_storage();
        // Short but valid Starknet address
        let c = sample_contact("Short", "0xdead", AddressType::Starknet);
        assert!(save_contact(&s, c).is_ok());
    }

    #[test]
    fn accepts_valid_oubli_128() {
        let s = mock_storage();
        let c = sample_contact("Full", OUBLI_PUBKEY_2, AddressType::Oubli);
        assert!(save_contact(&s, c).is_ok());
    }
}

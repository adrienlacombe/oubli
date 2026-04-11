use std::collections::HashMap;
use std::sync::Mutex;

use crate::platform::PlatformStorage;

/// In-memory mock for `PlatformStorage`, used in tests.
pub struct MockPlatformStorage {
    store: Mutex<HashMap<String, Vec<u8>>>,
    biometric_available: bool,
    biometric_response: bool,
}

impl MockPlatformStorage {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
            biometric_available: true,
            biometric_response: true,
        }
    }

    pub fn with_biometric(mut self, available: bool, response: bool) -> Self {
        self.biometric_available = available;
        self.biometric_response = response;
        self
    }
}

impl Default for MockPlatformStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformStorage for MockPlatformStorage {
    fn secure_store(&self, key: &str, value: &[u8]) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|e| e.to_string())?
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn secure_load(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self
            .store
            .lock()
            .map_err(|e| e.to_string())?
            .get(key)
            .cloned())
    }

    fn secure_delete(&self, key: &str) -> Result<(), String> {
        self.store.lock().map_err(|e| e.to_string())?.remove(key);
        Ok(())
    }

    fn request_biometric(&self, _reason: &str) -> Result<bool, String> {
        Ok(self.biometric_response)
    }

    fn biometric_available(&self) -> bool {
        self.biometric_available
    }

    fn generate_hardware_salt(&self) -> Result<Vec<u8>, String> {
        // Fixed salt for deterministic tests
        Ok(vec![0xAA; 16])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_load_delete() {
        let mock = MockPlatformStorage::new();
        mock.secure_store("key1", b"value1").unwrap();
        assert_eq!(mock.secure_load("key1").unwrap(), Some(b"value1".to_vec()));
        mock.secure_delete("key1").unwrap();
        assert_eq!(mock.secure_load("key1").unwrap(), None);
    }

    #[test]
    fn load_missing_returns_none() {
        let mock = MockPlatformStorage::new();
        assert_eq!(mock.secure_load("nonexistent").unwrap(), None);
    }

    #[test]
    fn biometric_mock() {
        let mock = MockPlatformStorage::new().with_biometric(true, false);
        assert!(mock.biometric_available());
        assert!(!mock.request_biometric("test").unwrap());
    }
}

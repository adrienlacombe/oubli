/// Platform-specific secure storage abstraction.
///
/// Implemented by native code (iOS Keychain / Android Keystore) and
/// injected into Rust via UniFFI callbacks.
pub trait PlatformStorage: Send + Sync {
    /// Store a value securely under the given key.
    fn secure_store(&self, key: &str, value: &[u8]) -> Result<(), String>;

    /// Load a previously stored value. Returns `None` if not found.
    fn secure_load(&self, key: &str) -> Result<Option<Vec<u8>>, String>;

    /// Delete a stored value.
    fn secure_delete(&self, key: &str) -> Result<(), String>;

    /// Request biometric authentication from the platform.
    /// Returns `true` if the user authenticated successfully.
    fn request_biometric(&self, reason: &str) -> Result<bool, String>;

    /// Whether biometric authentication is available on this device.
    fn biometric_available(&self) -> bool;

    /// Generate a hardware-backed random salt (e.g. from Secure Enclave / StrongBox).
    fn generate_hardware_salt(&self) -> Result<Vec<u8>, String>;
}

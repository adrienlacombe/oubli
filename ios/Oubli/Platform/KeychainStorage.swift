import Foundation
import Security
import LocalAuthentication

/// iOS Keychain + biometric implementation of `PlatformStorageCallback`.
///
/// All keychain items use:
/// - `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` (no iCloud sync, no locked-device access)
/// - `kSecAttrSynchronizable: false`
final class KeychainStorage: PlatformStorageCallback {

    // MARK: - Constants

    private let service = "com.oubli.wallet"

    // MARK: - PlatformStorageCallback

    func secureStore(key: String, value: [UInt8]) throws {
        let data = Data(value)

        // Delete any existing item first to avoid errSecDuplicateItem.
        let deleteQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
        ]
        SecItemDelete(deleteQuery as CFDictionary)

        let addQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
        ]

        let status = SecItemAdd(addQuery as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw OubliError.Store(message: "Keychain store failed: \(status)")
        }
    }

    func secureLoad(key: String) throws -> [UInt8]? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: kCFBooleanTrue as Any,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        switch status {
        case errSecSuccess:
            guard let data = result as? Data else {
                throw OubliError.Store(message: "Keychain returned non-data result")
            }
            return Array(data)
        case errSecItemNotFound:
            return nil
        default:
            throw OubliError.Store(message: "Keychain load failed: \(status)")
        }
    }

    func secureDelete(key: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
        ]

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw OubliError.Store(message: "Keychain delete failed: \(status)")
        }
    }

    func requestBiometric(reason: String) throws -> Bool {
        #if targetEnvironment(simulator)
        // Simulator has no biometric hardware — auto-succeed for development.
        return true
        #else
        let context = LAContext()
        var error: NSError?

        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) else {
            return false
        }

        // LAContext.evaluatePolicy is async via completion handler.
        // Bridge it to synchronous for the UniFFI callback expectation.
        var success = false
        var evalError: Error?
        let semaphore = DispatchSemaphore(value: 0)

        context.evaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics,
            localizedReason: reason
        ) { result, err in
            success = result
            evalError = err
            semaphore.signal()
        }

        semaphore.wait()

        if let evalError = evalError {
            // User cancelled or biometric failed -- not a hard error, just return false.
            let laError = evalError as NSError
            if laError.domain == LAError.errorDomain,
               laError.code == LAError.userCancel.rawValue ||
               laError.code == LAError.userFallback.rawValue ||
               laError.code == LAError.authenticationFailed.rawValue {
                return false
            }
            throw OubliError.Auth(message: "Biometric error: \(evalError.localizedDescription)")
        }

        return success
        #endif
    }

    func biometricAvailable() -> Bool {
        let context = LAContext()
        var error: NSError?
        return context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)
    }

    func generateHardwareSalt() throws -> [UInt8] {
        var bytes = [UInt8](repeating: 0, count: 32)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else {
            throw OubliError.Store(message: "SecRandomCopyBytes failed: \(status)")
        }
        return bytes
    }
}

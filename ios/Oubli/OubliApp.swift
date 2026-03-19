import SwiftUI
import Security

@main
struct OubliApp: App {
    @StateObject private var viewModel: WalletViewModel

    init() {
        // Allow UI tests to reset keychain state for a clean onboarding flow.
        if ProcessInfo.processInfo.arguments.contains("-reset-state") {
            Self.deleteAllKeychainItems()
        }

        let storage = KeychainStorage()
        let vm = WalletViewModel(storage: storage)
        _viewModel = StateObject(wrappedValue: vm)
    }

    private static func deleteAllKeychainItems() {
        let service = "com.oubli.wallet"
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
        ]
        SecItemDelete(query as CFDictionary)
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(viewModel)
        }
    }
}

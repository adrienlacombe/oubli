import Foundation
import SwiftUI
import os

private let logger = Logger(subsystem: "com.oubli.wallet", category: "WalletVM")

/// Main view-model bridging the Rust `OubliWallet` core to SwiftUI views.
///
/// All Rust FFI calls are dispatched to a serial background queue so the main
/// thread is never blocked by cryptographic operations, network calls, or
/// keychain access.
@MainActor
final class WalletViewModel: ObservableObject {

    // MARK: - Published state

    @Published private(set) var stateInfo: WalletStateInfo
    @Published var isBalanceHidden: Bool = false
    @Published var showUsd: Bool = false
    @Published private(set) var btcPriceUsd: Double? = nil
    @Published private(set) var initError: String?
    @Published private(set) var activity: [ActivityEventFfi] = []
    @Published private(set) var biometricUnlockError: String?

    // MARK: - Core

    private var wallet: OubliWallet?
    private let storage: PlatformStorageCallback
    private let backgroundQueue = DispatchQueue(label: "com.oubli.wallet.background", qos: .userInitiated)
    private var activityTimer: Timer?

    // MARK: - Derived helpers

    var currentState: WalletStateFfi { stateInfo.state }
    var address: String? { stateInfo.address }
    var publicKey: String? { stateInfo.publicKey }
    var balanceSats: String? { stateInfo.balanceSats }
    var pendingSats: String? { stateInfo.pendingSats }
    var operation: String? { stateInfo.operation }
    var errorMessage: String? { stateInfo.errorMessage }
    var autoFundError: String? { stateInfo.autoFundError }

    // MARK: - Init

    init(storage: PlatformStorageCallback) {
        self.storage = storage
        // Start with onboarding; real state is fetched after wallet init.
        self.stateInfo = WalletStateInfo(
            state: .onboarding,
            address: nil,
            publicKey: nil,
            balanceSats: nil,
            pendingSats: nil,
            operation: nil,
            errorMessage: nil,
            autoFundError: nil
        )

        initializeWallet(storage: storage)
    }

    private func initializeWallet(storage: PlatformStorageCallback) {
        backgroundQueue.async { [weak self] in
            do {
                let w = try OubliWallet(storage: storage)
                let state = w.getState()
                Task { @MainActor [weak self] in
                    self?.wallet = w
                    self?.applyState(state)
                }
            } catch {
                Task { @MainActor [weak self] in
                    self?.initError = error.localizedDescription
                }
            }
        }
    }

    // MARK: - State refresh

    private func applyState(_ state: WalletStateInfo) {
        self.stateInfo = state
        updateActivityPolling()
    }

    // MARK: - Activity polling

    private func updateActivityPolling() {
        let shouldPoll = stateInfo.state == .ready || stateInfo.state == .processing
        if shouldPoll && activityTimer == nil {
            refreshBtcPrice()
            activityTimer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.loadActivity()
                }
            }
        } else if !shouldPoll && activityTimer != nil {
            activityTimer?.invalidate()
            activityTimer = nil
        }
    }

    // MARK: - Background dispatch helper

    /// Runs a throwing closure on the background queue, then refreshes published state on main.
    private func dispatch(_ work: @escaping (OubliWallet) throws -> Void) {
        guard let wallet = wallet else { return }

        backgroundQueue.async { [weak self] in
            do {
                try work(wallet)
                let newState = wallet.getState()
                Task { @MainActor [weak self] in
                    self?.applyState(newState)
                }
            } catch {
                let errorState = WalletStateInfo(
                    state: .error,
                    address: nil,
                    publicKey: nil,
                    balanceSats: nil,
                    pendingSats: nil,
                    operation: nil,
                    errorMessage: error.localizedDescription,
                    autoFundError: nil
                )
                Task { @MainActor [weak self] in
                    self?.applyState(errorState)
                }
            }
        }
    }

    /// Like `dispatch` but returns a String result through a completion.
    private func dispatchWithResult(
        _ work: @escaping (OubliWallet) throws -> String,
        completion: ((Result<String, Error>) -> Void)? = nil
    ) {
        guard let wallet = wallet else { return }

        backgroundQueue.async { [weak self] in
            do {
                let result = try work(wallet)
                let newState = wallet.getState()
                Task { @MainActor [weak self] in
                    self?.applyState(newState)
                    completion?(.success(result))
                }
            } catch {
                let errorState = WalletStateInfo(
                    state: .error,
                    address: nil,
                    publicKey: nil,
                    balanceSats: nil,
                    pendingSats: nil,
                    operation: nil,
                    errorMessage: error.localizedDescription,
                    autoFundError: nil
                )
                Task { @MainActor [weak self] in
                    self?.applyState(errorState)
                    completion?(.failure(error))
                }
            }
        }
    }

    // MARK: - Onboarding

    func generateMnemonic() -> String? {
        guard let wallet = wallet else { return nil }
        do {
            return try wallet.generateMnemonic()
        } catch {
            stateInfo = WalletStateInfo(
                state: .error,
                address: nil,
                publicKey: nil,
                balanceSats: nil,
                pendingSats: nil,
                operation: nil,
                errorMessage: error.localizedDescription,
                autoFundError: nil
            )
            return nil
        }
    }

    func validateMnemonic(phrase: String) -> Bool {
        guard let wallet = wallet else { return false }
        do {
            try wallet.validateMnemonic(phrase: phrase)
            return true
        } catch {
            return false
        }
    }

    func completeOnboarding(mnemonic: String) {
        dispatch { wallet in
            try wallet.handleCompleteOnboarding(mnemonic: mnemonic)
        }
        loadActivity()
    }

    // MARK: - Unlock

    func unlockBiometric() {
        guard let wallet = wallet else {
            biometricUnlockError = "Wallet is unavailable. Restart the app and try again."
            return
        }
        biometricUnlockError = nil

        backgroundQueue.async { [weak self] in
            do {
                try wallet.handleUnlockBiometric()
                let newState = wallet.getState()
                let cached = wallet.getCachedActivity()
                Task { @MainActor [weak self] in
                    self?.applyState(newState)
                    self?.activity = cached
                    self?.biometricUnlockError = nil
                }
            } catch {
                let message = Self.biometricUnlockErrorMessage(from: error)
                Task { @MainActor [weak self] in
                    self?.biometricUnlockError = message
                }
            }
        }
    }

    // MARK: - Operations

    func send(amountSats: String, recipient: String, completion: ((String?) -> Void)? = nil) {
        dispatchWithResult { wallet in
            try wallet.handleSend(amountSats: amountSats, recipient: recipient)
        } completion: { [weak self] result in
            switch result {
            case .success(let txHash):
                self?.loadActivity()
                completion?(txHash)
            case .failure:
                completion?(nil)
            }
        }
    }

    func payLightning(bolt11: String, completion: ((String?) -> Void)? = nil) {
        dispatchWithResult { wallet in
            try wallet.payLightning(bolt11: bolt11)
        } completion: { [weak self] result in
            switch result {
            case .success(let txHash):
                self?.loadActivity()
                completion?(txHash)
            case .failure:
                completion?(nil)
            }
        }
    }

    func receiveLightningCreateInvoice(amountSats: UInt64, completion: @escaping (Result<SwapQuoteFfi, Error>) -> Void) {
        guard let wallet = wallet else {
            logger.error("receiveLightningCreateInvoice: wallet is nil")
            return
        }
        logger.info("receiveLightningCreateInvoice: amount=\(amountSats)")
        backgroundQueue.async {
            do {
                let quote = try wallet.swapLnToWbtc(amountSats: amountSats, exactIn: false)
                logger.info("receiveLightningCreateInvoice: got quote, swapId=\(quote.swapId)")
                Task { @MainActor in
                    completion(.success(quote))
                }
            } catch {
                logger.error("receiveLightningCreateInvoice: error: \(error)")
                Task { @MainActor in
                    completion(.failure(error))
                }
            }
        }
    }

    func receiveLightningWait(swapId: String, completion: @escaping (Result<Void, Error>) -> Void) {
        guard let wallet = wallet else {
            logger.error("receiveLightningWait: wallet is nil")
            return
        }
        logger.info("receiveLightningWait: starting wait for swapId=\(swapId)")
        backgroundQueue.async { [weak self] in
            do {
                logger.info("receiveLightningWait: calling bridge...")
                try wallet.receiveLightningWait(swapId: swapId)
                logger.info("receiveLightningWait: success!")
                let newState = wallet.getState()
                Task { @MainActor [weak self] in
                    self?.applyState(newState)
                    self?.loadActivity()
                    completion(.success(()))
                }
            } catch {
                logger.error("receiveLightningWait: error: \(error)")
                Task { @MainActor in
                    completion(.failure(error))
                }
            }
        }
    }

    func refreshBalance() {
        dispatch { wallet in
            try wallet.handleRefreshBalance()
        }
        loadActivity()
    }

    func loadActivity() {
        guard let wallet = wallet else { return }
        backgroundQueue.async { [weak self] in
            do {
                let events = try wallet.getActivity()
                Task { @MainActor [weak self] in
                    self?.activity = events
                }
            } catch {
                print("[Oubli] loadActivity error: \(error)")
            }
        }
    }

    func refreshBtcPrice() {
        guard let wallet = wallet else { return }
        backgroundQueue.async { [weak self] in
            let price = wallet.getBtcPriceUsd()
            Task { @MainActor [weak self] in
                if let price = price {
                    self?.btcPriceUsd = price
                }
            }
        }
    }

    func satsToUsd(_ sats: String) -> String? {
        guard let price = btcPriceUsd,
              let satsVal = Double(sats) else { return nil }
        let usd = satsVal * price / 100_000_000.0
        if usd < 0.01 {
            return String(format: "$%.4f", usd)
        }
        return String(format: "$%.2f", usd)
    }

    // MARK: - Seed Backup

    func startSeedBackup(mnemonic: String) -> SeedBackupStateFfi? {
        guard let wallet = wallet else { return nil }
        do {
            return try wallet.handleStartSeedBackup(mnemonic: mnemonic)
        } catch {
            stateInfo = WalletStateInfo(
                state: .error,
                address: nil,
                publicKey: nil,
                balanceSats: nil,
                pendingSats: nil,
                operation: nil,
                errorMessage: error.localizedDescription,
                autoFundError: nil
            )
            return nil
        }
    }

    func verifySeedWord(promptIndex: UInt32, answer: String) -> Bool {
        guard let wallet = wallet else { return false }
        do {
            return try wallet.handleVerifySeedWord(promptIndex: promptIndex, answer: answer)
        } catch {
            return false
        }
    }

    // MARK: - Seed Phrase Retrieval

    func getMnemonic(completion: @escaping (Result<String, Error>) -> Void) {
        guard let wallet = wallet else {
            completion(.failure(NSError(domain: "OubliWallet", code: -1, userInfo: [NSLocalizedDescriptionKey: "Wallet not initialized"])))
            return
        }

        backgroundQueue.async {
            do {
                let mnemonic = try wallet.getMnemonic()
                Task { @MainActor in
                    completion(.success(mnemonic))
                }
            } catch {
                Task { @MainActor in
                    completion(.failure(error))
                }
            }
        }
    }

    // MARK: - RPC URL (Debug Settings)

    func getRpcUrl() -> String {
        guard let wallet = wallet else { return "" }
        return wallet.getRpcUrl()
    }

    func updateRpcUrl(_ url: String) {
        guard let wallet = wallet else { return }
        wallet.updateRpcUrl(url: url)
    }

    // MARK: - Error dismissal

    func dismissError() {
        guard let wallet = wallet else { return }
        let state = wallet.getState()
        stateInfo = state
    }

    // MARK: - App reset

    func restartWallet() {
        wallet = nil
        initError = nil
        activity = []
        isBalanceHidden = false
        biometricUnlockError = nil
        stateInfo = WalletStateInfo(
            state: .onboarding,
            address: nil,
            publicKey: nil,
            balanceSats: nil,
            pendingSats: nil,
            operation: nil,
            errorMessage: nil,
            autoFundError: nil
        )
        initializeWallet(storage: storage)
    }

    nonisolated private static func biometricUnlockErrorMessage(from error: Error) -> String {
        let rawMessage: String

        if let oubliError = error as? OubliError {
            switch oubliError {
            case .Auth(let message), .InvalidState(let message):
                rawMessage = message
            default:
                rawMessage = error.localizedDescription
            }
        } else {
            rawMessage = error.localizedDescription
        }

        let normalized = rawMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        let lowercased = normalized.lowercased()

        if lowercased.contains("biometric authentication failed") || lowercased.contains("authentication failed") {
            return "Authentication failed. Try again."
        }
        if lowercased.contains("cancel") {
            return "Authentication was canceled. Try again."
        }
        if lowercased.contains("locked out") {
            return "Biometric authentication is temporarily locked. Use your device passcode, then try again."
        }
        if lowercased.contains("not available") {
            return "Biometric authentication is unavailable right now. Try again."
        }

        return normalized.isEmpty ? "Authentication failed. Try again." : normalized
    }
}

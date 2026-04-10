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
    @Published var showFiat: Bool = false
    @Published var fiatCurrency: String = UserDefaults.standard.string(forKey: "oubli_fiat_currency") ?? "usd"
    @Published private(set) var btcFiatPrice: Double? = nil
    @Published private(set) var initError: String?
    @Published private(set) var activity: [ActivityEventFfi] = []
    @Published private(set) var contacts: [ContactFfi] = []
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

        initializeWallet()
    }

    private func initializeWallet() {
        let workItem = DispatchWorkItem { [weak self] in
            guard let self else { return }
            do {
                let w = try OubliWallet(storage: self.storage, rpcUrl: nil, paymasterApiKey: nil)
                let state = w.getState()
                Task { @MainActor in
                    self.wallet = w
                    self.applyState(state)
                }
            } catch {
                Task { @MainActor in
                    self.initError = error.localizedDescription
                }
            }
        }
        backgroundQueue.async(execute: workItem)
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
            loadContacts()
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
        loadContacts()
        refreshBtcPrice()
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
                    self?.refreshBtcPrice()
                }
            } catch {
                let message = Self.biometricUnlockErrorMessage(from: error)
                Task { @MainActor [weak self] in
                    self?.biometricUnlockError = message
                }
            }
        }
    }

    // MARK: - Fee

    func calculateFee(amountSats: String) -> String {
        guard let w = wallet else { return "0" }
        return w.calculateFee(amountSats: amountSats)
    }

    func calculateSendFee(amountSats: String, recipient: String) -> String {
        guard let w = wallet else { return "0" }
        return w.calculateSendFee(amountSats: amountSats, recipient: recipient)
    }

    func feePercent() -> Double {
        guard let w = wallet else { return 0.0 }
        return w.getFeePercent()
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

    // MARK: - Contacts

    func loadContacts() {
        guard let wallet = wallet else { return }
        backgroundQueue.async { [weak self] in
            let list = wallet.getContacts()
            Task { @MainActor [weak self] in
                self?.contacts = list
            }
        }
    }

    func saveContact(_ contact: ContactFfi) {
        guard let wallet = wallet else { return }
        backgroundQueue.async { [weak self] in
            _ = try? wallet.saveContact(contact: contact)
            let list = wallet.getContacts()
            Task { @MainActor [weak self] in
                self?.contacts = list
            }
        }
    }

    func deleteContact(id: String) {
        guard let wallet = wallet else { return }
        backgroundQueue.async { [weak self] in
            try? wallet.deleteContact(contactId: id)
            let list = wallet.getContacts()
            Task { @MainActor [weak self] in
                self?.contacts = list
            }
        }
    }

    func findContactByAddress(_ address: String) -> ContactFfi? {
        wallet?.findContactByAddress(address: address)
    }

    func updateContactLastUsed(id: String) {
        guard let wallet = wallet else { return }
        backgroundQueue.async { [weak self] in
            try? wallet.updateContactLastUsed(contactId: id)
            let list = wallet.getContacts()
            Task { @MainActor [weak self] in
                self?.contacts = list
            }
        }
    }

    func getTransferRecipient(txHash: String) -> String? {
        wallet?.getTransferRecipient(txHash: txHash)
    }

    func refreshBtcPrice() {
        guard let wallet = wallet else { return }
        let currency = fiatCurrency
        backgroundQueue.async { [weak self] in
            let price = wallet.getBtcPrice(currency: currency)
            Task { @MainActor [weak self] in
                if let price = price {
                    self?.btcFiatPrice = price
                }
            }
        }
    }

    func setFiatCurrency(_ code: String) {
        fiatCurrency = code.lowercased()
        UserDefaults.standard.set(fiatCurrency, forKey: "oubli_fiat_currency")
        btcFiatPrice = nil
        refreshBtcPrice()
    }

    /// Format sats as fiat using the cached BTC price.
    func satsToFiat(_ sats: String) -> String? {
        guard let price = btcFiatPrice,
              let satsVal = Double(sats) else { return nil }
        let fiat = satsVal * price / 100_000_000.0
        let symbol = Self.fiatSymbol(for: fiatCurrency)
        if fiat < 0.01 {
            return String(format: "\(symbol)%.4f", fiat)
        }
        return String(format: "\(symbol)%.2f", fiat)
    }

    /// Raw numeric fiat value (no symbol) for a given sats amount.
    func satsToFiatRaw(_ sats: String) -> String? {
        guard let price = btcFiatPrice,
              let satsVal = Double(sats), satsVal > 0 else { return nil }
        let fiat = satsVal * price / 100_000_000.0
        if fiat < 0.01 {
            return String(format: "%.4f", fiat)
        }
        return String(format: "%.2f", fiat)
    }

    /// Convert a fiat amount string to sats (rounded to nearest integer).
    func fiatToSats(_ fiat: String) -> String? {
        guard let price = btcFiatPrice, price > 0,
              let fiatVal = Double(fiat), fiatVal > 0 else { return nil }
        let sats = fiatVal / price * 100_000_000.0
        return String(Int(sats.rounded()))
    }

    static let supportedFiatCurrencies: [(code: String, name: String)] = [
        ("usd", "US Dollar"),
        ("eur", "Euro"),
        ("gbp", "British Pound"),
        ("jpy", "Japanese Yen"),
        ("cad", "Canadian Dollar"),
        ("aud", "Australian Dollar"),
        ("chf", "Swiss Franc"),
        ("cny", "Chinese Yuan"),
        ("inr", "Indian Rupee"),
        ("brl", "Brazilian Real"),
        ("krw", "Korean Won"),
        ("mxn", "Mexican Peso"),
        ("try", "Turkish Lira"),
        ("sek", "Swedish Krona"),
        ("nok", "Norwegian Krone"),
        ("dkk", "Danish Krone"),
        ("pln", "Polish Zloty"),
        ("zar", "South African Rand"),
        ("thb", "Thai Baht"),
        ("sgd", "Singapore Dollar"),
        ("hkd", "Hong Kong Dollar"),
        ("nzd", "New Zealand Dollar"),
    ]

    static func fiatSymbol(for code: String) -> String {
        switch code.lowercased() {
        case "usd", "cad", "aud", "nzd", "sgd", "hkd", "mxn": return "$"
        case "eur": return "€"
        case "gbp": return "£"
        case "jpy", "cny": return "¥"
        case "inr": return "₹"
        case "brl": return "R$"
        case "krw": return "₩"
        case "try": return "₺"
        case "sek", "nok", "dkk": return "kr "
        case "pln": return "zł "
        case "zar": return "R "
        case "thb": return "฿"
        case "chf": return "CHF "
        default: return "\(code.uppercased()) "
        }
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
        initializeWallet()
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
            return "Biometric authentication is temporarily locked. Wait a moment, then try again."
        }
        if lowercased.contains("not available") {
            return "Biometric authentication is unavailable right now. Try again."
        }

        return normalized.isEmpty ? "Authentication failed. Try again." : normalized
    }
}

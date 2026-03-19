import SwiftUI
import CoreImage

/// Main dashboard showing wallet balance and action buttons.
struct BalanceView: View {
    @EnvironmentObject var viewModel: WalletViewModel

    @State private var showSendSheet: Bool = false
    @State private var showReceiveSheet: Bool = false
    @State private var showSeedPhraseSheet: Bool = false
    @State private var showDebugSettings: Bool = false
    @State private var toastMessage: String? = nil

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 24) {
                    balanceCard
                    actionButtons
                    activitySection
                    autoFundErrorBanner
                }
                .padding(.bottom, 24)
                .padding(.top, 8)
            }
            .navigationBarHidden(true)
            .refreshable {
                viewModel.refreshBalance()
            }
            .fullScreenCover(isPresented: $showSendSheet) {
                SendSheet(viewModel: viewModel) { txHash in
                    if let hash = txHash {
                        let short = hash.count > 16
                            ? String(hash.prefix(10)) + "..." + String(hash.suffix(6))
                            : hash
                        withAnimation { toastMessage = "Sent: \(short)" }
                    }
                }
            }
            .fullScreenCover(isPresented: $showReceiveSheet) {
                ReceiveSheet(address: viewModel.address ?? "", publicKey: viewModel.publicKey ?? "", viewModel: viewModel)
            }
            .fullScreenCover(isPresented: $showSeedPhraseSheet) {
                SeedPhraseSheet(viewModel: viewModel)
            }
            .sheet(isPresented: $showDebugSettings) {
                DebugSettingsSheet(viewModel: viewModel)
            }
            .overlay(alignment: .bottom) {
                if let message = toastMessage {
                    Text(message)
                        .font(.subheadline)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                        .background(.ultraThinMaterial, in: Capsule())
                        .padding(.bottom, 32)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                        .onAppear {
                            DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
                                withAnimation { toastMessage = nil }
                            }
                        }
                }
            }
            .animation(.easeInOut, value: toastMessage)
        }
    }

    // MARK: - Balance Card

    private var balanceCard: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(spacing: 8) {
                if viewModel.isBalanceHidden {
                    Text("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}")
                        .font(.system(size: 40, weight: .bold, design: .monospaced))
                        .onTapGesture { viewModel.isBalanceHidden.toggle() }
                } else if viewModel.showUsd, let usd = viewModel.satsToUsd(viewModel.balanceSats ?? "0") {
                    Text(usd)
                        .font(.system(size: 40, weight: .bold, design: .monospaced))
                        .onTapGesture { viewModel.isBalanceHidden.toggle() }
                } else {
                    Text(viewModel.balanceSats ?? "0")
                        .font(.system(size: 40, weight: .bold, design: .monospaced))
                        .onTapGesture { viewModel.isBalanceHidden.toggle() }
                }

                Text(viewModel.showUsd ? "USD" : "sats")
                    .font(.title3)
                    .foregroundColor(.secondary)
                    .onTapGesture {
                        viewModel.showUsd.toggle()
                        if viewModel.showUsd && viewModel.btcPriceUsd == nil {
                            viewModel.refreshBtcPrice()
                        }
                    }

                if !viewModel.isBalanceHidden {
                    if let pending = viewModel.pendingSats, pending != "0" {
                        HStack(spacing: 4) {
                            Image(systemName: "clock")
                                .font(.caption)
                            if viewModel.showUsd, let usd = viewModel.satsToUsd(pending) {
                                Text("\(usd) incoming")
                                    .font(.caption)
                            } else {
                                Text("\(pending) sats incoming")
                                    .font(.caption)
                            }
                        }
                        .foregroundColor(.orange)
                        .padding(.top, 4)
                    }
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 32)
            .background(Color(.systemGray6))
            .cornerRadius(16)

            Menu {
                Button {
                    showSeedPhraseSheet = true
                } label: {
                    Label("Show Seed Phrase", systemImage: "key.viewfinder")
                }
                Button {
                    showDebugSettings = true
                } label: {
                    Label("Debug Settings", systemImage: "gearshape")
                }
            } label: {
                Image(systemName: "ellipsis.circle")
                    .font(.title2)
                    .foregroundColor(.secondary)
            }
            .accessibilityIdentifier("moreMenu")
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Action Buttons

    private var actionButtons: some View {
        HStack(spacing: 12) {
            Button {
                showSendSheet = true
            } label: {
                Label("Send", systemImage: "arrow.up.circle")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)

            Button {
                showReceiveSheet = true
            } label: {
                Label("Receive", systemImage: "arrow.down.circle")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Activity Section

    private var activitySection: some View {
        VStack(spacing: 12) {
            HStack {
                Text("Activity")
                    .font(.headline)
                Spacer()
            }

            if viewModel.activity.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "clock")
                        .font(.system(size: 32))
                        .foregroundColor(.secondary)
                    Text("No transactions yet")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
                .background(Color(.systemGray6))
                .cornerRadius(12)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(viewModel.activity.enumerated()), id: \.offset) { index, event in
                        if index > 0 {
                            Divider()
                        }
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(activityLabel(event.eventType))
                                    .font(.subheadline.weight(.medium))
                                Text(shortHash(event.txHash))
                                    .font(.caption.monospaced())
                                    .foregroundColor(.secondary)
                            }
                            Spacer()
                            if let amount = event.amountSats {
                                Text("\(amount) sats")
                                    .font(.subheadline.monospaced().weight(.medium))
                            }
                        }
                        .padding(.vertical, 10)
                        .padding(.horizontal, 12)
                        .accessibilityIdentifier("activityRow_\(index)")
                    }
                }
                .background(Color(.systemGray6))
                .cornerRadius(12)
                .accessibilityIdentifier("activityList")
            }
        }
        .padding(.horizontal, 24)
    }

    private func activityLabel(_ type: String) -> String {
        switch type {
        case "Fund": return "Received"
        case "TransferOut": return "Sent"
        case "TransferIn": return "Received"
        case "Withdraw": return "Sent"
        case "Rollover": return "Settled"
        case "Ragequit": return "Emergency Exit"
        default: return type
        }
    }

    private func shortHash(_ hash: String) -> String {
        if hash.count > 16 {
            return String(hash.prefix(10)) + "..." + String(hash.suffix(6))
        }
        return hash
    }

    // MARK: - Auto-fund Error Banner

    @ViewBuilder
    private var autoFundErrorBanner: some View {
        if let error = viewModel.autoFundError {
            VStack(alignment: .leading, spacing: 4) {
                Text("Auto-fund error (tap to copy)")
                    .font(.caption.weight(.bold))
                Text(error)
                    .font(.caption2)
            }
            .foregroundColor(.white)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(Color.red.opacity(0.85))
            .cornerRadius(12)
            .padding(.horizontal, 24)
            .onTapGesture {
                UIPasteboard.general.string = error
                withAnimation { toastMessage = "Copied to clipboard" }
            }
        }
    }
}

// MARK: - Debug Settings Sheet

private struct DebugSettingsSheet: View {
    @ObservedObject var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var rpcUrl: String = ""

    var body: some View {
        NavigationStack {
            Form {
                Section("Starknet RPC Endpoint") {
                    TextField("RPC URL", text: $rpcUrl)
                        .font(.body.monospaced())
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                }
            }
            .navigationTitle("Debug Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        viewModel.updateRpcUrl(rpcUrl)
                        dismiss()
                    }
                    .disabled(rpcUrl.isEmpty)
                }
            }
            .onAppear {
                rpcUrl = viewModel.getRpcUrl()
            }
        }
        .presentationDetents([.medium])
    }
}

// MARK: - Send Sheet

private enum SwapProcessState: Equatable {
    case idle
    case processing
    case success(String)
    case error(String)
}

private struct SendSheet: View {
    @ObservedObject var viewModel: WalletViewModel
    var onSent: ((String?) -> Void)?
    @Environment(\.dismiss) private var dismiss
    @State private var amount: String = ""
    @State private var recipient: String = ""
    @State private var showConfirmation: Bool = false
    @State private var showScanner: Bool = false
    @State private var showNoAmountAlert: Bool = false
    @State private var swapState: SwapProcessState = .idle

    private var balanceString: String {
        viewModel.balanceSats ?? "0"
    }

    private var insufficientFunds: Bool {
        guard let entered = Int64(amount),
              let available = Int64(balanceString) else { return false }
        return entered > available
    }

    private var normalizedLightningInvoice: String? {
        normalizeLightningInvoice(recipient)
    }

    private var lightningInvoiceAmountSats: String? {
        guard let invoice = normalizedLightningInvoice else { return nil }
        return parseBolt11AmountSats(invoice)
    }

    private var hasLightningInvoice: Bool {
        normalizedLightningInvoice != nil
    }

    private var lightningInvoiceMissingAmount: Bool {
        hasLightningInvoice && lightningInvoiceAmountSats == nil
    }

    private var canReview: Bool {
        !recipient.isEmpty && !amount.isEmpty && !insufficientFunds && !lightningInvoiceMissingAmount
    }

    var body: some View {
        NavigationStack {
            currentContent
                .navigationTitle(screenTitle)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    if let leadingActionTitle {
                        ToolbarItem(placement: .cancellationAction) {
                            Button(leadingActionTitle) {
                                handleLeadingAction()
                            }
                            .disabled(!canDismiss)
                        }
                    }
                }
        }
        .interactiveDismissDisabled(swapState == .processing)
        .safeAreaInset(edge: .bottom) {
            bottomActionBar
        }
        .sheet(isPresented: $showScanner) {
            QRScannerSheet(onCodeScanned: { code in
                handleScannedCode(code)
            }, isPresented: $showScanner)
        }
        .alert("No Amount in Invoice", isPresented: $showNoAmountAlert) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("This Lightning invoice doesn't include an amount. Please ask the recipient for an invoice with a specific amount.")
        }
        .onChange(of: recipient) { newValue in
            syncAmountWithRecipient(newValue)
        }
    }

    @ViewBuilder
    private var currentContent: some View {
        switch swapState {
        case .processing:
            swapProcessingView
        case .success(let message):
            swapResultView(success: true, message: message)
        case .error(let message):
            swapResultView(success: false, message: message)
        case .idle:
            if showConfirmation {
                confirmationView
            } else {
                sendFormView
            }
        }
    }

    private var screenTitle: String {
        switch swapState {
        case .processing:
            return "Lightning Payment"
        case .success:
            return "Payment Sent"
        case .error:
            return "Payment Failed"
        case .idle:
            if showConfirmation {
                return hasLightningInvoice ? "Confirm Payment" : "Confirm Send"
            }
            return "Send"
        }
    }

    private var leadingActionTitle: String? {
        switch swapState {
        case .processing:
            return nil
        case .idle where showConfirmation:
            return "Back"
        default:
            return "Close"
        }
    }

    private var canDismiss: Bool {
        swapState != .processing
    }

    private func handleLeadingAction() {
        if showConfirmation && swapState == .idle {
            showConfirmation = false
        } else if canDismiss {
            dismiss()
        }
    }

    // MARK: - Swap Processing View

    private var swapProcessingView: some View {
        VStack(spacing: 24) {
            Spacer()
            ProgressView()
                .scaleEffect(1.5)
            Text("Processing Lightning payment...")
                .font(.headline)
            Text(viewModel.stateInfo.operation ?? "This may take a few minutes")
                .font(.subheadline)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Swap Result View

    private func swapResultView(success: Bool, message: String) -> some View {
        VStack(spacing: 20) {
            Spacer()
            Image(systemName: success ? "checkmark.circle.fill" : "xmark.circle.fill")
                .font(.system(size: 56))
                .foregroundColor(success ? .green : .red)
            Text(success ? "Payment Sent" : "Payment Failed")
                .font(.title2.weight(.semibold))
            Text(message)
                .font(.callout)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Confirmation View

    private var confirmationView: some View {
        Form {
            Section("Review") {
                if hasLightningInvoice {
                    LabeledContent("Payment", value: "Lightning invoice")
                }
                LabeledContent("Amount", value: "\(amount) sats")
                LabeledContent("To", value: shortRecipient)
            }
        }
    }

    // MARK: - Send Form View

    private var sendFormView: some View {
        Form {
            Section("Amount (sats)") {
                HStack {
                    TextField("0", text: $amount)
                        .keyboardType(.numberPad)
                        .font(.body.monospaced())
                        .disabled(hasLightningInvoice)
                    if !hasLightningInvoice {
                        Button("Max") {
                            amount = balanceString
                        }
                        .font(.body.bold())
                    }
                }
                if hasLightningInvoice {
                    Text(lightningInvoiceMissingAmount
                         ? "This Lightning invoice doesn't include an amount."
                         : "Amount comes from the Lightning invoice.")
                        .font(.caption)
                        .foregroundColor(lightningInvoiceMissingAmount ? .red : .secondary)
                }
                if insufficientFunds {
                    Text("Insufficient funds (available: \(balanceString))")
                        .font(.caption)
                        .foregroundColor(.red)
                }
            }
            Section("Recipient") {
                HStack {
                    TextField("Address or Lightning invoice", text: $recipient)
                        .font(.body.monospaced())
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                    Button {
                        handlePaste()
                    } label: {
                        Image(systemName: "doc.on.clipboard")
                            .font(.title3)
                    }
                    Button {
                        showScanner = true
                    } label: {
                        Image(systemName: "qrcode.viewfinder")
                            .font(.title2)
                    }
                }
                if hasLightningInvoice {
                    Label("Lightning invoice detected", systemImage: "bolt.fill")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
    }

    @ViewBuilder
    private var bottomActionBar: some View {
        switch swapState {
        case .processing:
            EmptyView()
        case .success:
            actionBar(buttonTitle: "Done", enabled: true) {
                onSent?(nil)
                dismiss()
            }
        case .error:
            actionBar(buttonTitle: "Close", enabled: true) {
                dismiss()
            }
        case .idle:
            if showConfirmation {
                actionBar(buttonTitle: hasLightningInvoice ? "Pay Invoice" : "Send", enabled: canReview) {
                    if let invoice = normalizedLightningInvoice {
                        startLightningPayment(bolt11: invoice)
                    } else {
                        viewModel.send(amountSats: amount, recipient: recipient) { txHash in
                            onSent?(txHash)
                        }
                        dismiss()
                    }
                }
            } else {
                actionBar(
                    buttonTitle: "Review",
                    enabled: canReview
                ) {
                    showConfirmation = true
                }
            }
        }
    }

    private func actionBar(buttonTitle: String, enabled: Bool, action: @escaping () -> Void) -> some View {
        VStack(spacing: 0) {
            Divider()
            Button(action: action) {
                Text(buttonTitle)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(!enabled)
            .padding(.horizontal, 24)
            .padding(.top, 16)
            .padding(.bottom, 12)
            .background(.ultraThinMaterial)
        }
    }

    private var shortRecipient: String {
        if recipient.count > 20 {
            return String(recipient.prefix(10)) + "..." + String(recipient.suffix(6))
        }
        return recipient
    }

    private func handlePaste() {
        guard let text = UIPasteboard.general.string?.trimmingCharacters(in: .whitespacesAndNewlines),
              !text.isEmpty else { return }
        handleScannedCode(text)
    }

    private func handleScannedCode(_ code: String) {
        if let normalized = normalizeLightningInvoice(code) {
            if let parsedAmount = parseBolt11AmountSats(normalized) {
                recipient = normalized
                amount = parsedAmount
                showConfirmation = true
            } else {
                showNoAmountAlert = true
            }
        } else if let parsed = parseOubliUri(code) {
            recipient = parsed.pubkey
            if let sats = parsed.amount {
                amount = sats
            }
        } else {
            recipient = code.trimmingCharacters(in: .whitespacesAndNewlines)
        }
    }

    private func parseOubliUri(_ code: String) -> (pubkey: String, amount: String?)? {
        let trimmed = code.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.lowercased().hasPrefix("oubli:") else { return nil }
        let rest = String(trimmed.dropFirst(6))
        if let qIndex = rest.firstIndex(of: "?") {
            let pubkey = String(rest[rest.startIndex..<qIndex])
            let query = String(rest[rest.index(after: qIndex)...])
            let params = query.components(separatedBy: "&")
            var amount: String? = nil
            for param in params {
                let parts = param.components(separatedBy: "=")
                if parts.count == 2 && parts[0] == "amount" {
                    amount = parts[1]
                }
            }
            return (pubkey, amount)
        }
        return (rest, nil)
    }

    private func startLightningPayment(bolt11: String) {
        swapState = .processing
        viewModel.payLightning(bolt11: bolt11) { txHash in
            if let hash = txHash {
                let short = hash.count > 16
                    ? String(hash.prefix(10)) + "..." + String(hash.suffix(6))
                    : hash
                swapState = .success("Tx: \(short)")
            } else {
                let errorMsg = viewModel.stateInfo.errorMessage ?? "Unknown error"
                swapState = .error(errorMsg)
            }
        }
    }

    private func syncAmountWithRecipient(_ value: String) {
        guard let invoice = normalizeLightningInvoice(value),
              let parsedAmount = parseBolt11AmountSats(invoice),
              amount != parsedAmount else { return }
        amount = parsedAmount
    }

    private func normalizeLightningInvoice(_ value: String) -> String? {
        let normalized = value.lowercased()
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "lightning:", with: "")
        guard normalized.hasPrefix("lnbc")
                || normalized.hasPrefix("lntb")
                || normalized.hasPrefix("lnbcrt") else {
            return nil
        }
        return normalized
    }

    private func parseBolt11AmountSats(_ invoice: String) -> String? {
        let lower = invoice.lowercased()
        let rest: String
        if lower.hasPrefix("lnbcrt") {
            rest = String(lower.dropFirst(6))
        } else if lower.hasPrefix("lnbc") {
            rest = String(lower.dropFirst(4))
        } else if lower.hasPrefix("lntb") {
            rest = String(lower.dropFirst(4))
        } else {
            return nil
        }
        guard let separatorIndex = rest.firstIndex(of: "1") else { return nil }
        let amountPart = String(rest[..<separatorIndex])
        guard !amountPart.isEmpty else { return nil }

        let multiplier: Character?
        let digitsString: String
        if let last = amountPart.last, "munp".contains(last) {
            multiplier = last
            digitsString = String(amountPart.dropLast())
        } else {
            multiplier = nil
            digitsString = amountPart
        }

        guard !digitsString.isEmpty, let base = Decimal(string: digitsString) else { return nil }

        let sats: Decimal
        switch multiplier {
        case "m":
            sats = base * Decimal(100_000)
        case "u":
            sats = base * Decimal(100)
        case "n":
            sats = base / Decimal(10)
        case "p":
            sats = base / Decimal(10_000)
        default:
            sats = base * Decimal(100_000_000)
        }

        var mutableSats = sats
        var roundedSats = Decimal()
        NSDecimalRound(&roundedSats, &mutableSats, 0, .plain)
        let result = NSDecimalNumber(decimal: roundedSats)
        guard result != .notANumber, result.int64Value > 0 else { return nil }
        return result.stringValue
    }
}

// MARK: - QR Scanner Sheet

private struct QRScannerSheet: View {
    var onCodeScanned: (String) -> Void
    @Binding var isPresented: Bool

    var body: some View {
        NavigationStack {
            QRScannerView { code in
                isPresented = false
                onCodeScanned(code)
            }
            .ignoresSafeArea()
            .navigationTitle("Scan QR Code")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { isPresented = false }
                }
            }
        }
    }
}

// MARK: - Receive Sheet

private struct ReceiveSheet: View {
    private enum ReceiveTab: Int, CaseIterable {
        case oubli
        case starknet
        case lightning

        var title: String {
            switch self {
            case .oubli:
                return "Oubli"
            case .starknet:
                return "Starknet"
            case .lightning:
                return "Lightning"
            }
        }
    }

    let address: String
    let publicKey: String
    @ObservedObject var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var selectedTab: ReceiveTab = .oubli
    @State private var toastMessage: String? = nil

    // Oubli receive amount (optional)
    @State private var oubliAmountSats: String = ""

    // Lightning receive state
    @State private var lnAmountSats: String = ""
    @State private var lnInvoice: String? = nil
    @State private var lnSwapId: String? = nil
    @State private var lnFee: String? = nil
    @State private var lnWaiting: Bool = false
    @State private var lnSuccess: Bool = false
    @State private var lnError: String? = nil

    var body: some View {
        NavigationStack {
            VStack(spacing: 16) {
                Picker("Receive type", selection: $selectedTab) {
                    ForEach(ReceiveTab.allCases, id: \.self) { tab in
                        Text(tab.title).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, 24)
                .padding(.top, 12)

                TabView(selection: $selectedTab) {
                    oubliReceiveCard
                    .tag(ReceiveTab.oubli)

                    receiveCard(
                        title: "Starknet",
                        subtitle: "For receiving from any Starknet wallet",
                        value: address,
                        icon: "globe"
                    )
                    .tag(ReceiveTab.starknet)

                    lightningReceiveCard
                        .tag(ReceiveTab.lightning)
                }
                .tabViewStyle(.page(indexDisplayMode: .never))
            }
            .navigationTitle("Receive")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                        .disabled(lnWaiting)
                }
            }
        }
        .interactiveDismissDisabled(lnWaiting)
        .overlay(alignment: .bottom) {
            if let message = toastMessage {
                Text(message)
                    .font(.subheadline)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(.ultraThinMaterial, in: Capsule())
                    .padding(.bottom, 32)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .onAppear {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
                            withAnimation { toastMessage = nil }
                        }
                    }
            }
        }
        .animation(.easeInOut, value: toastMessage)
    }

    // MARK: - Oubli Receive Card

    private var oubliReceiveValue: String {
        if oubliAmountSats.isEmpty {
            return publicKey
        }
        return "oubli:\(publicKey)?amount=\(oubliAmountSats)"
    }

    private var oubliReceiveCard: some View {
        VStack(spacing: 12) {
            HStack(spacing: 6) {
                Image(systemName: "lock.shield")
                    .font(.caption)
                Text("Oubli")
                    .font(.subheadline.weight(.semibold))
            }
            .foregroundColor(.secondary)

            if let qrImage = generateQRCode(from: oubliReceiveValue) {
                Image(uiImage: qrImage)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: 200, maxHeight: 200)
            }

            Text("For receiving from Oubli wallets")
                .font(.caption)
                .foregroundColor(.secondary)

            Text(publicKey)
                .font(.caption2.monospaced())
                .multilineTextAlignment(.center)
                .lineLimit(3)
                .padding(.horizontal, 32)

            TextField("Amount (sats, optional)", text: $oubliAmountSats)
                .keyboardType(.numberPad)
                .textFieldStyle(.roundedBorder)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            requestActionRow(
                copyLabel: "Copy",
                copyValue: oubliReceiveValue,
                shareLabel: "Share",
                shareMessage: receiveShareMessage(title: "Oubli", subtitle: "For receiving from Oubli wallets", value: oubliReceiveValue),
                controlSize: .regular
            )
        }
        .frame(maxHeight: .infinity)
    }

    // MARK: - Lightning Receive Card

    private var lightningReceiveCard: some View {
        VStack(spacing: 16) {
            Spacer()

            HStack(spacing: 6) {
                Image(systemName: "bolt.fill")
                    .font(.caption)
                Text("Lightning")
                    .font(.subheadline.weight(.semibold))
            }
            .foregroundColor(.secondary)

            if lnSuccess {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 56))
                    .foregroundColor(.green)
                Text("Payment Received!")
                    .font(.headline)
                Text("WBTC will be auto-funded into your privacy pool.")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
            } else if lnWaiting {
                if let invoice = lnInvoice {
                    if let qrImage = generateQRCode(from: invoice) {
                        Image(uiImage: qrImage)
                            .interpolation(.none)
                            .resizable()
                            .scaledToFit()
                            .frame(width: 180, height: 180)
                    }
                    Text(String(invoice.prefix(30)) + "...")
                        .font(.caption2.monospaced())
                        .foregroundColor(.secondary)
                    requestActionRow(
                        copyLabel: "Copy Invoice",
                        copyValue: invoice,
                        shareLabel: "Share Invoice",
                        shareMessage: lightningShareMessage(invoice: invoice),
                        controlSize: .small
                    )
                }
                HStack(spacing: 8) {
                    ProgressView()
                    Text("Waiting for payment...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                if let fee = lnFee {
                    Text("Fee: \(fee) sats")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            } else if let invoice = lnInvoice {
                if let qrImage = generateQRCode(from: invoice) {
                    Image(uiImage: qrImage)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 200, height: 200)
                }
                Text("Share this invoice with the sender")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Text(String(invoice.prefix(30)) + "...")
                    .font(.caption2.monospaced())
                    .foregroundColor(.secondary)
                requestActionRow(
                    copyLabel: "Copy Invoice",
                    copyValue: invoice,
                    shareLabel: "Share Invoice",
                    shareMessage: lightningShareMessage(invoice: invoice),
                    controlSize: .regular
                )

                if let errorMsg = lnError {
                    Text(errorMsg)
                        .font(.caption)
                        .foregroundColor(.red)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 32)

                    if lnSwapId != nil {
                        Button("Retry Payment Check") {
                            retryLnWait()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                    }
                }

                if let fee = lnFee {
                    Text("Fee: \(fee) sats")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            } else {
                // Amount input + create invoice
                if let errorMsg = lnError {
                    Text(errorMsg)
                        .font(.caption)
                        .foregroundColor(.red)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 32)
                }

                Text("Enter amount to receive via Lightning")
                    .font(.caption)
                    .foregroundColor(.secondary)

                HStack {
                    TextField("Amount", text: $lnAmountSats)
                        .keyboardType(.numberPad)
                        .font(.body.monospaced())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 150)
                    Text("sats")
                        .foregroundColor(.secondary)
                }

                Button("Create Invoice") {
                    createLnInvoice()
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(lnAmountSats.isEmpty || UInt64(lnAmountSats) == nil || UInt64(lnAmountSats) == 0)
            }

            Spacer()
        }
    }

    private func createLnInvoice() {
        guard let amount = UInt64(lnAmountSats), amount > 0 else { return }
        lnError = nil

        viewModel.receiveLightningCreateInvoice(amountSats: amount) { result in
            switch result {
            case .success(let quote):
                lnInvoice = quote.lnInvoice
                lnSwapId = quote.swapId
                lnFee = quote.fee
                // Automatically start waiting for payment
                if let swapId = quote.swapId as String? {
                    waitForLnPayment(swapId: swapId)
                }
            case .failure(let error):
                lnError = error.localizedDescription
            }
        }
    }

    private func retryLnWait() {
        guard let swapId = lnSwapId else { return }
        waitForLnPayment(swapId: swapId)
    }

    private func waitForLnPayment(swapId: String) {
        lnError = nil
        lnWaiting = true
        viewModel.receiveLightningWait(swapId: swapId) { waitResult in
            lnWaiting = false
            switch waitResult {
            case .success:
                lnSuccess = true
                viewModel.refreshBalance()
            case .failure(let error):
                lnError = error.localizedDescription
            }
        }
    }

    // MARK: - Static Receive Card

    private func receiveCard(title: String, subtitle: String, value: String, icon: String) -> some View {
        VStack(spacing: 16) {
            Spacer()

            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.caption)
                Text(title)
                    .font(.subheadline.weight(.semibold))
            }
            .foregroundColor(.secondary)

            if let qrImage = generateQRCode(from: value) {
                Image(uiImage: qrImage)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 200, height: 200)
            }

            Text(subtitle)
                .font(.caption)
                .foregroundColor(.secondary)

            Text(value)
                .font(.caption2.monospaced())
                .multilineTextAlignment(.center)
                .lineLimit(3)
                .padding(.horizontal, 32)

            requestActionRow(
                copyLabel: "Copy",
                copyValue: value,
                shareLabel: "Share",
                shareMessage: receiveShareMessage(title: title, subtitle: subtitle, value: value),
                controlSize: .regular
            )

            Spacer()
        }
    }

    private func requestActionRow(
        copyLabel: String,
        copyValue: String,
        shareLabel: String,
        shareMessage: String,
        controlSize: ControlSize
    ) -> some View {
        HStack(spacing: 12) {
            Button {
                UIPasteboard.general.string = copyValue
                withAnimation { toastMessage = "Copied to clipboard" }
            } label: {
                Label(copyLabel, systemImage: "doc.on.doc")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(controlSize)

            ShareLink(item: shareMessage) {
                Label(shareLabel, systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(controlSize)
        }
        .padding(.horizontal, 24)
    }

    private func receiveShareMessage(title: String, subtitle: String, value: String) -> String {
        let descriptor = title == "Starknet" ? "Address" : "Public key"
        return """
        Receive with Oubli
        Type: \(title)
        \(subtitle)
        \(descriptor): \(value)
        """
    }

    private func lightningShareMessage(invoice: String) -> String {
        let amount = lightningInvoiceAmountSats(invoice).map { "\($0) sats" } ?? "Custom amount"
        return """
        Pay me on Lightning with Oubli
        Amount: \(amount)
        Invoice: \(invoice)
        """
    }

    private func lightningInvoiceAmountSats(_ invoice: String) -> String? {
        let lower = invoice.lowercased()
        let rest: String
        if lower.hasPrefix("lnbcrt") {
            rest = String(lower.dropFirst(6))
        } else if lower.hasPrefix("lnbc") {
            rest = String(lower.dropFirst(4))
        } else if lower.hasPrefix("lntb") {
            rest = String(lower.dropFirst(4))
        } else {
            return nil
        }
        guard let separatorIndex = rest.firstIndex(of: "1") else { return nil }
        let amountPart = String(rest[..<separatorIndex])
        guard !amountPart.isEmpty else { return nil }

        let multiplier: Character?
        let digitsString: String
        if let last = amountPart.last, "munp".contains(last) {
            multiplier = last
            digitsString = String(amountPart.dropLast())
        } else {
            multiplier = nil
            digitsString = amountPart
        }

        guard !digitsString.isEmpty, let base = Decimal(string: digitsString) else { return nil }

        let sats: Decimal
        switch multiplier {
        case "m":
            sats = base * Decimal(100_000)
        case "u":
            sats = base * Decimal(100)
        case "n":
            sats = base / Decimal(10)
        case "p":
            sats = base / Decimal(10_000)
        default:
            sats = base * Decimal(100_000_000)
        }

        var mutableSats = sats
        var roundedSats = Decimal()
        NSDecimalRound(&roundedSats, &mutableSats, 0, .plain)
        let result = NSDecimalNumber(decimal: roundedSats)
        guard result != .notANumber, result.int64Value > 0 else { return nil }
        return result.stringValue
    }

    private func generateQRCode(from string: String) -> UIImage? {
        let context = CIContext()
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(Data(string.utf8), forKey: "inputMessage")
        guard let outputImage = filter.outputImage else { return nil }
        let scaledImage = outputImage.transformed(by: CGAffineTransform(scaleX: 10, y: 10))
        guard let cgImage = context.createCGImage(scaledImage, from: scaledImage.extent) else { return nil }
        return UIImage(cgImage: cgImage)
    }
}

// MARK: - Seed Phrase Sheet

private struct SeedPhraseSheet: View {
    @ObservedObject var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var seedWords: [String]? = nil
    @State private var error: String? = nil
    @State private var isLoading: Bool = false
    @State private var copied: Bool = false
    @State private var toastMessage: String? = nil

    var body: some View {
        NavigationStack {
            seedPhraseContent
                .navigationTitle(seedWords == nil ? "Show Seed Phrase" : "Seed Phrase")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Close") { dismiss() }
                            .disabled(isLoading)
                    }
                }
        }
        .interactiveDismissDisabled(isLoading)
        .safeAreaInset(edge: .bottom) {
            VStack(spacing: 0) {
                Divider()
                Button(seedWords == nil ? (isLoading ? "Loading..." : "Reveal") : "Done") {
                    if seedWords == nil {
                        revealSeed()
                    } else {
                        dismiss()
                    }
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(isLoading)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 24)
                .padding(.top, 16)
                .padding(.bottom, 12)
                .background(.ultraThinMaterial)
            }
        }
        .overlay(alignment: .bottom) {
            if let message = toastMessage {
                Text(message)
                    .font(.subheadline)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(.ultraThinMaterial, in: Capsule())
                    .padding(.bottom, 32)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .onAppear {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
                            withAnimation { toastMessage = nil }
                        }
                    }
            }
        }
        .animation(.easeInOut, value: toastMessage)
    }

    @ViewBuilder
    private var seedPhraseContent: some View {
        if let words = seedWords {
            ScrollView {
                VStack(spacing: 16) {
                    Image(systemName: "exclamationmark.shield")
                        .font(.system(size: 40))
                        .foregroundColor(.red)

                    Text("Write down these words in order and store them safely. Anyone with these words can access your funds.")
                        .font(.callout)
                        .foregroundColor(.red)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)

                    VStack(alignment: .leading, spacing: 8) {
                        ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                            HStack {
                                Text("\(index + 1).")
                                    .font(.body.monospaced())
                                    .foregroundColor(.secondary)
                                    .frame(width: 30, alignment: .trailing)
                                Text(word)
                                    .font(.body.monospaced())
                            }
                        }
                    }
                    .padding(20)
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    .padding(.horizontal, 24)
                    .accessibilityIdentifier("seedWordsList")

                    Button {
                        UIPasteboard.general.string = words.joined(separator: " ")
                        copied = true
                        withAnimation { toastMessage = "Copied to clipboard" }
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                            copied = false
                        }
                    } label: {
                        Label(copied ? "Copied!" : "Copy to Clipboard", systemImage: copied ? "checkmark" : "doc.on.doc")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.large)
                    .padding(.horizontal, 24)
                }
                .padding(.vertical, 24)
            }
        } else {
            VStack(spacing: 24) {
                Spacer()

                if isLoading {
                    ProgressView("Loading...")
                } else {
                    Image(systemName: "key.viewfinder")
                        .font(.system(size: 48))
                        .foregroundStyle(.secondary)

                    Text("Reveal your seed phrase")
                        .font(.headline)

                    if let error = error {
                        Text(error)
                            .font(.callout)
                            .foregroundColor(.red)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, 24)
                    }
                }

                Spacer()
            }
        }
    }

    private func revealSeed() {
        isLoading = true
        error = nil
        viewModel.getMnemonic { result in
            isLoading = false
            switch result {
            case .success(let mnemonic):
                seedWords = mnemonic.split(separator: " ").map(String.init)
            case .failure(let err):
                error = err.localizedDescription
            }
        }
    }
}

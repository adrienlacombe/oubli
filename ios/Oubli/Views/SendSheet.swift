import SwiftUI

// MARK: - Swap Process State

enum SwapProcessState: Equatable {
    case idle
    case processing
    case success(String)
    case error(String)
}

// MARK: - Send Sheet

struct SendSheet: View {
    @ObservedObject var viewModel: WalletViewModel
    var initialCode: String? = nil
    var onSent: ((String?) -> Void)?
    @Environment(\.dismiss) private var dismiss
    @State private var amount: String = ""
    @State private var recipient: String = ""
    @State private var showScanner: Bool = false
    @State private var showNoAmountAlert: Bool = false
    @State private var swapState: SwapProcessState = .idle
    @State private var resultIconScale: CGFloat = 0.3
    @State private var processingRotation: Double = 0
    @State private var showSaveContactSheet: Bool = false
    @State private var showSendConfirmation: Bool = false

    @State private var debouncedFeeSats: String = "0"
    @State private var feeDebounceTask: DispatchWorkItem? = nil

    private var balanceString: String {
        viewModel.balanceSats ?? "0"
    }

    private var feeSats: String { debouncedFeeSats }

    private var totalSats: Int64 {
        let a = Int64(amount) ?? 0
        let f = Int64(feeSats) ?? 0
        return a + f
    }

    private var insufficientFunds: Bool {
        guard let available = Int64(balanceString) else { return false }
        return totalSats > available
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
        .alert("Confirm Send", isPresented: $showSendConfirmation) {
            Button("Send", role: .destructive) {
                executeOnChainSend()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            let display = recipient.count > 28
                ? String(recipient.prefix(16)) + "..." + String(recipient.suffix(8))
                : recipient
            Text("Send \(amount) sats to \(display)?")
        }
        .onChange(of: recipient) { newValue in
            syncAmountWithRecipient(newValue)
            debounceFeeCalculation()
        }
        .onChange(of: amount) { _ in
            debounceFeeCalculation()
        }
        .onAppear {
            if let code = initialCode {
                handleScannedCode(code)
            }
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
            sendFormView
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
            return "Send"
        }
    }

    private var leadingActionTitle: String? {
        switch swapState {
        case .processing:
            return nil
        default:
            return "Close"
        }
    }

    private var canDismiss: Bool {
        swapState != .processing
    }

    private func handleLeadingAction() {
        if canDismiss {
            dismiss()
        }
    }

    // MARK: - Swap Processing View

    private var swapProcessingView: some View {
        VStack(spacing: 24) {
            Spacer()
            ZStack {
                Circle()
                    .stroke(Color.oubliPrimary.opacity(0.2), lineWidth: 4)
                    .frame(width: 80, height: 80)
                Circle()
                    .trim(from: 0, to: 0.7)
                    .stroke(Color.oubliPrimary, style: StrokeStyle(lineWidth: 4, lineCap: .round))
                    .frame(width: 80, height: 80)
                    .rotationEffect(.degrees(processingRotation))
                Image(systemName: "bolt.fill")
                    .font(.system(size: 28))
                    .foregroundStyle(Color.oubliPrimary)
            }
            .accessibilityLabel("Processing payment")
            Text("Processing Lightning payment...")
                .font(.headline)
            Text(viewModel.stateInfo.operation ?? "This may take a few minutes")
                .font(.subheadline)
                .foregroundColor(Color.oubliOnSurfaceVariant)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear {
            withAnimation(.linear(duration: 1.0).repeatForever(autoreverses: false)) {
                processingRotation = 360
            }
        }
    }

    // MARK: - Swap Result View

    private var recipientIsKnownContact: Bool {
        let trimmed = recipient.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return true }
        return viewModel.findContactByAddress(trimmed) != nil
    }

    private func swapResultView(success: Bool, message: String) -> some View {
        VStack(spacing: 20) {
            Spacer()
            Image(systemName: success ? "checkmark.circle.fill" : "xmark.circle.fill")
                .font(.system(size: 72))
                .foregroundStyle(.white)
                .scaleEffect(resultIconScale)
                .accessibilityHidden(true)
            Text(success ? "Payment Sent" : "Payment Failed")
                .font(.title2.weight(.semibold))
                .foregroundStyle(.white)
                .accessibilityAddTraits(.isHeader)
            Text(message)
                .font(.callout)
                .foregroundColor(.white.opacity(0.8))
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
            if success && !recipientIsKnownContact {
                Button {
                    showSaveContactSheet = true
                } label: {
                    Label("Save to Contacts", systemImage: "person.crop.circle.badge.plus")
                        .font(.subheadline.weight(.medium))
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                        .background(.white.opacity(0.2), in: Capsule())
                        .foregroundStyle(.white)
                }
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(success ? Color.oubliReceived : Color.oubliError)
        .onAppear {
            if success {
                UINotificationFeedbackGenerator().notificationOccurred(.success)
            } else {
                UINotificationFeedbackGenerator().notificationOccurred(.error)
            }
            withAnimation(.spring(response: 0.5, dampingFraction: 0.6)) {
                resultIconScale = 1.0
            }
        }
        .sheet(isPresented: $showSaveContactSheet) {
            ContactDetailSheet(
                viewModel: viewModel,
                contact: prefillContactForRecipient()
            )
        }
    }

    private func prefillContactForRecipient() -> ContactFfi {
        let trimmed = recipient.trimmingCharacters(in: .whitespacesAndNewlines)
        let stripped = trimmed.hasPrefix("0x") ? String(trimmed.dropFirst(2)) : trimmed
        let addrType: AddressTypeFfi = stripped.count > 64 ? .oubli : .starknet
        return ContactFfi(
            id: "",
            name: "",
            addresses: [ContactAddressFfi(address: trimmed, addressType: addrType, label: nil)],
            notes: nil,
            createdAt: 0,
            lastUsedAt: 0
        )
    }

    // MARK: - Send Form View

    private var sendFormView: some View {
        Form {
            Section("Amount") {
                DualAmountInput(
                    satsAmount: $amount,
                    satsToFiatRaw: { viewModel.satsToFiatRaw($0) },
                    fiatToSats: { viewModel.fiatToSats($0) },
                    fiatCurrency: viewModel.fiatCurrency,
                    fiatSymbol: WalletViewModel.fiatSymbol(for: viewModel.fiatCurrency),
                    isReadOnly: hasLightningInvoice,
                    showMaxButton: !hasLightningInvoice,
                    maxSats: balanceString
                )
                if hasLightningInvoice {
                    Text(lightningInvoiceMissingAmount
                         ? "This Lightning invoice doesn't include an amount."
                         : "Amount comes from the Lightning invoice.")
                        .font(.caption)
                        .foregroundColor(lightningInvoiceMissingAmount ? .red : .secondary)
                }
                if insufficientFunds {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.triangle")
                            .foregroundColor(Color.oubliError)
                            .accessibilityHidden(true)
                        if let fee = Int64(feeSats), fee > 0 {
                            Text("Insufficient funds (need \(totalSats), available: \(balanceString))")
                                .font(.caption)
                                .foregroundColor(Color.oubliError)
                        } else {
                            Text("Insufficient funds (available: \(balanceString))")
                                .font(.caption)
                                .foregroundColor(Color.oubliError)
                        }
                    }
                }
            }
            if !amount.isEmpty, let fee = Int64(feeSats), fee > 0 {
                Section("Summary") {
                    LabeledContent("Est. fee", value: "\(feeSats) sats")
                    LabeledContent("Total", value: "\(totalSats) sats")
                        .fontWeight(.semibold)
                    if let totalFiat = viewModel.satsToFiat(String(totalSats)) {
                        LabeledContent("", value: totalFiat)
                            .foregroundColor(Color.oubliOnSurfaceVariant)
                    }
                }
            }
            Section("Recipient") {
                ContactPickerView(viewModel: viewModel) { address in
                    recipient = address
                }
                HStack {
                    TextField("Address or Lightning invoice", text: $recipient)
                        .font(.body.monospaced())
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .accessibilityLabel("Recipient address or Lightning invoice")
                    Button {
                        handlePaste()
                    } label: {
                        Image(systemName: "doc.on.clipboard")
                            .font(.title3)
                    }
                    .accessibilityLabel("Paste from clipboard")
                    Button {
                        showScanner = true
                    } label: {
                        Image(systemName: "qrcode.viewfinder")
                            .font(.title2)
                    }
                    .accessibilityLabel("Scan QR code")
                }
                if hasLightningInvoice {
                    Label("Lightning invoice detected", systemImage: "bolt.fill")
                        .font(.caption)
                        .foregroundColor(Color.oubliOnSurfaceVariant)
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
            actionBar(buttonTitle: hasLightningInvoice ? "Pay Invoice" : "Send", enabled: canReview) {
                if let invoice = normalizedLightningInvoice {
                    startLightningPayment(bolt11: invoice)
                } else {
                    showSendConfirmation = true
                }
            }
        }
    }

    private func actionBar(buttonTitle: String, buttonIcon: String? = nil, enabled: Bool, action: @escaping () -> Void) -> some View {
        VStack(spacing: 0) {
            Divider()
            Button(action: action) {
                HStack {
                    Text(buttonTitle)
                    if let icon = buttonIcon {
                        Image(systemName: icon)
                    }
                }
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

    private func executeOnChainSend() {
        swapState = .processing
        viewModel.send(amountSats: amount, recipient: recipient) { txHash in
            if let hash = txHash {
                let short = hash.count > 16
                    ? String(hash.prefix(10)) + "..." + String(hash.suffix(6))
                    : hash
                swapState = .success("Tx: \(short)")
                onSent?(txHash)
            } else {
                let errorMsg = viewModel.stateInfo.errorMessage ?? "Send failed"
                swapState = .error(errorMsg)
            }
        }
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

    private func debounceFeeCalculation() {
        feeDebounceTask?.cancel()
        let task = DispatchWorkItem { [amount, recipient] in
            guard !amount.isEmpty else {
                debouncedFeeSats = "0"
                return
            }
            debouncedFeeSats = viewModel.calculateSendFee(amountSats: amount, recipient: recipient)
        }
        feeDebounceTask = task
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: task)
    }

    private func syncAmountWithRecipient(_ value: String) {
        guard let invoice = normalizeLightningInvoice(value),
              let parsedAmount = parseBolt11AmountSats(invoice),
              amount != parsedAmount else { return }
        amount = parsedAmount
    }

    // MARK: - Lightning Invoice Helpers

    func normalizeLightningInvoice(_ value: String) -> String? {
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

    func parseBolt11AmountSats(_ invoice: String) -> String? {
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

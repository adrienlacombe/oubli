import SwiftUI
import CoreImage

// MARK: - Receive Sheet

struct ReceiveSheet: View {
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
    // Starknet receive amount (optional)
    @State private var starknetAmountSats: String = ""

    // Lightning receive state
    @State private var lnAmountSats: String = ""
    @State private var lnInvoice: String? = nil
    @State private var lnSwapId: String? = nil
    @State private var lnFee: String? = nil
    @State private var lnWaiting: Bool = false
    @State private var lnSuccess: Bool = false
    @State private var lnError: String? = nil
    @State private var lnExpiry: UInt64? = nil
    @State private var lnExpiryRemaining: Int = 0
    @State private var lnExpiryTimer: Timer? = nil

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
                .accessibilityLabel("Receive method")

                TabView(selection: $selectedTab) {
                    oubliReceiveCard
                    .tag(ReceiveTab.oubli)

                    starknetReceiveCard
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
                        announceToVoiceOver(message)
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
            if let qrImage = generateQRCode(from: oubliReceiveValue, logo: UIImage(systemName: "lock.shield.fill")) {
                Image(uiImage: qrImage)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: 220, maxHeight: 220)
                    .aspectRatio(1, contentMode: .fit)
                    .padding(12)
                    .background(
                        LinearGradient(
                            colors: [Color(hex: 0xE5E2E1), .white],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .cornerRadius(12)
                    .accessibilityLabel("QR code for Oubli address")
            }

            Text(charWrapped(publicKey))
                .font(.caption2.monospaced())
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 32)
                .accessibilityLabel("Public key: \(publicKey)")

            DualAmountInput(
                satsAmount: $oubliAmountSats,
                satsToFiatRaw: { viewModel.satsToFiatRaw($0) },
                fiatToSats: { viewModel.fiatToSats($0) },
                fiatCurrency: viewModel.fiatCurrency,
                fiatSymbol: WalletViewModel.fiatSymbol(for: viewModel.fiatCurrency)
            )
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

    // MARK: - Starknet Receive Card

    private var starknetReceiveValue: String {
        if starknetAmountSats.isEmpty {
            return address
        }
        return "oubli:\(address)?amount=\(starknetAmountSats)"
    }

    private var starknetReceiveCard: some View {
        VStack(spacing: 12) {
            if let qrImage = generateQRCode(from: starknetReceiveValue, logo: UIImage(systemName: "diamond.fill")) {
                Image(uiImage: qrImage)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: 220, maxHeight: 220)
                    .aspectRatio(1, contentMode: .fit)
                    .padding(12)
                    .background(
                        LinearGradient(
                            colors: [Color(hex: 0xE5E2E1), .white],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .cornerRadius(12)
                    .accessibilityLabel("QR code for Starknet address")
            }

            Text(charWrapped(address))
                .font(.caption2.monospaced())
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 32)
                .accessibilityLabel("Starknet address: \(address)")

            DualAmountInput(
                satsAmount: $starknetAmountSats,
                satsToFiatRaw: { viewModel.satsToFiatRaw($0) },
                fiatToSats: { viewModel.fiatToSats($0) },
                fiatCurrency: viewModel.fiatCurrency,
                fiatSymbol: WalletViewModel.fiatSymbol(for: viewModel.fiatCurrency)
            )
            .padding(.horizontal, 32)

            requestActionRow(
                copyLabel: "Copy",
                copyValue: starknetReceiveValue,
                shareLabel: "Share",
                shareMessage: receiveShareMessage(title: "Starknet", subtitle: "For receiving from any Starknet wallet", value: starknetReceiveValue),
                controlSize: .regular
            )
        }
        .frame(maxHeight: .infinity)
    }

    // MARK: - Lightning Receive Card

    private var lightningReceiveCard: some View {
        VStack(spacing: 16) {
            Spacer()

            if lnSuccess {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 56))
                    .foregroundStyle(Color.oubliReceived)
                    .accessibilityHidden(true)
                Text("Payment Received!")
                    .font(.headline)
                    .accessibilityAddTraits(.isHeader)
            } else if lnWaiting {
                if let invoice = lnInvoice {
                    if let qrImage = generateQRCode(from: invoice, logo: UIImage(systemName: "bolt.fill")) {
                        Image(uiImage: qrImage)
                            .interpolation(.none)
                            .resizable()
                            .scaledToFit()
                            .frame(maxWidth: 200, maxHeight: 200)
                            .aspectRatio(1, contentMode: .fit)
                            .accessibilityLabel("QR code for Lightning invoice")
                    }
                    Text(String(invoice.prefix(30)) + "...")
                        .font(.caption2.monospaced())
                        .foregroundColor(Color.oubliOnSurfaceVariant)
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
                        .accessibilityHidden(true)
                    Image(systemName: "clock")
                        .foregroundStyle(Color.oubliPending)
                        .accessibilityHidden(true)
                    Text("Waiting for payment...")
                        .font(.caption)
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                }
                .accessibilityElement(children: .combine)
                .accessibilityLabel("Waiting for payment")
                if lnExpiryRemaining > 0 {
                    let minutes = lnExpiryRemaining / 60
                    let seconds = lnExpiryRemaining % 60
                    Text("Expires in \(minutes):\(String(format: "%02d", seconds))")
                        .font(.caption2.monospacedDigit())
                        .foregroundColor(lnExpiryRemaining < 60 ? Color.oubliError : Color.oubliOnSurfaceVariant)
                }
                if let fee = lnFee {
                    Text("Fee: \(fee) sats")
                        .font(.caption2)
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                }
            } else if let invoice = lnInvoice {
                if let qrImage = generateQRCode(from: invoice, logo: UIImage(systemName: "bolt.fill")) {
                    Image(uiImage: qrImage)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(maxWidth: 220, maxHeight: 220)
                        .aspectRatio(1, contentMode: .fit)
                        .accessibilityLabel("QR code for Lightning invoice")
                }
                Text("Share this invoice with the sender")
                    .font(.caption)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                Text(String(invoice.prefix(30)) + "...")
                    .font(.caption2.monospaced())
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                requestActionRow(
                    copyLabel: "Copy Invoice",
                    copyValue: invoice,
                    shareLabel: "Share Invoice",
                    shareMessage: lightningShareMessage(invoice: invoice),
                    controlSize: .regular
                )

                if let errorMsg = lnError {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.triangle")
                            .foregroundStyle(Color.oubliError)
                            .accessibilityHidden(true)
                        Text(errorMsg)
                            .font(.caption)
                            .foregroundColor(Color.oubliError)
                    }
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
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                }
            } else {
                // Amount input + create invoice
                if let errorMsg = lnError {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.triangle")
                            .foregroundStyle(Color.oubliError)
                            .accessibilityHidden(true)
                        Text(errorMsg)
                            .font(.caption)
                            .foregroundColor(Color.oubliError)
                    }
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
                }

                Text("Enter amount to receive via Lightning")
                    .font(.caption)
                    .foregroundColor(Color.oubliOnSurfaceVariant)

                DualAmountInput(
                    satsAmount: $lnAmountSats,
                    satsToFiatRaw: { viewModel.satsToFiatRaw($0) },
                    fiatToSats: { viewModel.fiatToSats($0) },
                    fiatCurrency: viewModel.fiatCurrency,
                    fiatSymbol: WalletViewModel.fiatSymbol(for: viewModel.fiatCurrency)
                )
                .padding(.horizontal, 32)

                Button("Create Invoice") {
                    createLnInvoice()
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(lnAmountSats.isEmpty || UInt64(lnAmountSats) == nil || UInt64(lnAmountSats) == 0)
            }

            Spacer()
        }
        .onAppear {
            if lnSuccess {
                UINotificationFeedbackGenerator().notificationOccurred(.success)
            }
        }
        .onChange(of: lnSuccess) { newValue in
            if newValue {
                UINotificationFeedbackGenerator().notificationOccurred(.success)
            }
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
                lnExpiry = quote.expiry
                startExpiryCountdown(expiry: quote.expiry)
                // Automatically start waiting for payment
                if let swapId = quote.swapId as String? {
                    waitForLnPayment(swapId: swapId)
                }
            case .failure(let error):
                lnError = viewModel.userFacingMessage(for: error, context: .lightningReceive)
                UINotificationFeedbackGenerator().notificationOccurred(.error)
            }
        }
    }

    private func retryLnWait() {
        guard let swapId = lnSwapId else { return }
        waitForLnPayment(swapId: swapId)
    }

    private func startExpiryCountdown(expiry: UInt64) {
        lnExpiryTimer?.invalidate()
        let remaining = Int(Int64(expiry) - Int64(Date().timeIntervalSince1970))
        lnExpiryRemaining = max(0, remaining)
        lnExpiryTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { timer in
            let r = Int(Int64(expiry) - Int64(Date().timeIntervalSince1970))
            if r <= 0 {
                timer.invalidate()
                lnExpiryRemaining = 0
                if !lnSuccess {
                    lnWaiting = false
                    lnSwapId = nil
                    lnError = "Invoice expired. Create a new one."
                }
            } else {
                lnExpiryRemaining = r
            }
        }
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
                lnError = viewModel.userFacingMessage(for: error, context: .lightningReceive)
                UINotificationFeedbackGenerator().notificationOccurred(.error)
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
                    .accessibilityHidden(true)
                Text(title)
                    .font(.subheadline.weight(.semibold))
            }
            .foregroundColor(Color.oubliOnSurfaceVariant)

            if let qrImage = generateQRCode(from: value) {
                Image(uiImage: qrImage)
                    .interpolation(.none)
                    .resizable()
                    .scaledToFit()
                    .frame(maxWidth: 220, maxHeight: 220)
                    .aspectRatio(1, contentMode: .fit)
                    .accessibilityLabel("QR code for \(title) address")
            }

            Text(subtitle)
                .font(.caption)
                .foregroundColor(Color.oubliOnSurfaceVariant)

            Text(value)
                .font(.caption2.monospaced())
                .multilineTextAlignment(.center)
                .lineLimit(3)
                .padding(.horizontal, 32)
                .accessibilityLabel("\(title) address: \(value)")

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
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
                withAnimation { toastMessage = "Copied to clipboard" }
            } label: {
                Label(copyLabel, systemImage: "doc.on.doc")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .controlSize(controlSize)
            .accessibilityHint("Copies address to clipboard")

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

    /// Inserts zero-width spaces between characters so Text wraps by character without hyphens.
    private func charWrapped(_ string: String) -> String {
        string.map { String($0) }.joined(separator: "\u{200B}")
    }

    // MARK: - QR Code Generation

    func generateQRCode(from string: String, logo: UIImage? = nil) -> UIImage? {
        let context = CIContext()
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(Data(string.utf8), forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel") // Medium error correction for logo overlay
        guard let outputImage = filter.outputImage else { return nil }
        let scaledImage = outputImage.transformed(by: CGAffineTransform(scaleX: 10, y: 10))
        guard let cgImage = context.createCGImage(scaledImage, from: scaledImage.extent) else { return nil }
        let qrImage = UIImage(cgImage: cgImage)

        guard let logo = logo else { return qrImage }

        // Overlay logo centered on QR code
        let size = qrImage.size
        let logoSize = CGSize(width: size.width * 0.2, height: size.height * 0.2)
        let logoOrigin = CGPoint(x: (size.width - logoSize.width) / 2, y: (size.height - logoSize.height) / 2)

        UIGraphicsBeginImageContextWithOptions(size, false, 0)
        qrImage.draw(in: CGRect(origin: .zero, size: size))

        // White circle background behind logo
        let padding: CGFloat = 8
        let bgRect = CGRect(
            x: logoOrigin.x - padding,
            y: logoOrigin.y - padding,
            width: logoSize.width + padding * 2,
            height: logoSize.height + padding * 2
        )
        UIColor.white.setFill()
        UIBezierPath(ovalIn: bgRect).fill()

        logo.draw(in: CGRect(origin: logoOrigin, size: logoSize))
        let result = UIGraphicsGetImageFromCurrentImageContext()
        UIGraphicsEndImageContext()
        return result
    }

    // MARK: - VoiceOver Announcement

    private func announceToVoiceOver(_ message: String) {
        UIAccessibility.post(notification: .announcement, argument: message)
    }
}

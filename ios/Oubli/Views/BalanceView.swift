import SwiftUI

/// Pulses opacity between 1.0 and 0.4 to indicate pending state.
/// Respects Reduce Motion accessibility setting.
private struct PulseOpacity: ViewModifier {
    @State private var isPulsing = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        if reduceMotion {
            content.opacity(0.7)
        } else {
            content
                .opacity(isPulsing ? 0.4 : 1.0)
                .animation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true), value: isPulsing)
                .onAppear { isPulsing = true }
        }
    }
}

private func activityIsIncoming(_ type: String) -> Bool {
    switch type {
    case "Fund", "TransferIn", "Rollover":
        return true
    default:
        return false
    }
}

private struct SelectedActivity: Identifiable {
    let id: String
    let event: ActivityEventFfi
}

private struct ActivityStatusBadge: View {
    let status: String

    var body: some View {
        Text(status)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(backgroundColor)
            .foregroundColor(foregroundColor)
            .clipShape(Capsule())
    }

    private var normalizedStatus: String {
        let trimmed = status.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            return trimmed
        }
        return "Unknown"
    }

    private var backgroundColor: Color {
        switch normalizedStatus {
        case "Confirmed":
            return Color.oubliReceived.opacity(0.18)
        case "Pending":
            return Color.oubliPending.opacity(0.2)
        case "Failed":
            return Color.oubliError.opacity(0.18)
        default:
            return Color.oubliSurfaceContainerHigh
        }
    }

    private var foregroundColor: Color {
        switch normalizedStatus {
        case "Confirmed":
            return Color.oubliReceived
        case "Pending":
            return Color.oubliPending
        case "Failed":
            return Color.oubliError
        default:
            return Color.oubliOnSurfaceVariant
        }
    }
}

private struct ActivityDetailSheet: View {
    let event: ActivityEventFfi
    let title: String
    let contactName: String?
    let recipient: String?
    let onCopyHash: () -> Void

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section {
                    VStack(alignment: .leading, spacing: 12) {
                        ActivityStatusBadge(status: normalizedStatus)
                        if let amount = event.amountSats {
                            Text("\(signedAmountLabel(amount: amount)) sats")
                                .font(.title3.monospaced().weight(.semibold))
                        }
                        Text(title)
                            .font(.headline)
                        if let subtitle = counterpartyLabel {
                            Text(subtitle)
                                .font(.subheadline)
                                .foregroundColor(Color.oubliOnSurfaceVariant)
                        }
                    }
                    .padding(.vertical, 4)
                }

                Section("Details") {
                    detailRow(label: "Status", value: normalizedStatus)
                    if let timestamp = formattedTimestamp {
                        detailRow(label: "Time", value: timestamp)
                    }
                    if event.blockNumber > 0 {
                        detailRow(label: "Block", value: String(event.blockNumber))
                    }
                    if let recipient {
                        detailRow(label: "Address", value: recipient)
                    }
                }

                Section("Transaction Hash") {
                    Text(event.txHash)
                        .font(.footnote.monospaced())
                        .textSelection(.enabled)
                    Button("Copy Transaction Hash", action: onCopyHash)
                }

                if let explorerURL {
                    Section {
                        Link("View on Explorer", destination: explorerURL)
                    }
                }
            }
            .navigationTitle("Transaction")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private func detailRow(label: String, value: String) -> some View {
        LabeledContent(label) {
            Text(value)
                .font(label == "Address" ? .footnote.monospaced() : .body)
                .multilineTextAlignment(.trailing)
                .textSelection(.enabled)
        }
    }

    private var normalizedStatus: String {
        let trimmed = event.status.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            return trimmed
        }
        return event.blockNumber == 0 ? "Pending" : "Confirmed"
    }

    private var formattedTimestamp: String? {
        guard let timestamp = event.timestampSecs else {
            return normalizedStatus == "Pending" ? "Awaiting confirmation" : nil
        }
        return Self.dateFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(timestamp)))
    }

    private var explorerURL: URL? {
        guard let raw = event.explorerUrl else { return nil }
        return URL(string: raw)
    }

    private var counterpartyLabel: String? {
        if let contactName, !contactName.isEmpty {
            return contactName
        }
        if let recipient, !recipient.isEmpty {
            return recipient
        }
        return nil
    }

    private func signedAmountLabel(amount: String) -> String {
        activityIsIncoming(event.eventType) ? "+\(amount)" : "-\(amount)"
    }

    static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()
}

/// Main dashboard showing wallet balance and action buttons.
struct BalanceView: View {
    @EnvironmentObject var viewModel: WalletViewModel

    @State private var showSendSheet: Bool = false
    @State private var showReceiveSheet: Bool = false
    @State private var showSeedPhraseSheet: Bool = false
    @State private var showContactList: Bool = false
    @State private var showQRScanner: Bool = false
    @State private var scannedCode: String? = nil
    @State private var toastMessage: String? = nil
    @State private var fiatPriceTimedOut: Bool = false
    @State private var selectedActivity: SelectedActivity? = nil

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 24) {
                    hamburgerMenu
                    balanceCard
                    actionButtons
                    activitySection
                    autoFundErrorBanner
                }
                .padding(.bottom, 24)
            }
            .navigationBarHidden(true)
            .refreshable {
                viewModel.refreshBalance()
            }
            .sheet(isPresented: $showSendSheet, onDismiss: { scannedCode = nil }) {
                SendSheet(viewModel: viewModel, initialCode: scannedCode) { txHash in
                    if let hash = txHash {
                        let short = hash.count > 16
                            ? String(hash.prefix(10)) + "..." + String(hash.suffix(6))
                            : hash
                        withAnimation { toastMessage = "Sent: \(short)" }
                    }
                }
                .presentationDetents([.large])
            }
            .sheet(isPresented: $showQRScanner) {
                QRScannerSheet(onCodeScanned: { code in
                    scannedCode = code
                    showSendSheet = true
                }, isPresented: $showQRScanner)
            }
            .sheet(item: $selectedActivity) { selection in
                ActivityDetailSheet(
                    event: selection.event,
                    title: activityLabel(selection.event.eventType),
                    contactName: contactNameForTx(selection.event.txHash),
                    recipient: viewModel.getTransferRecipient(txHash: selection.event.txHash),
                    onCopyHash: {
                        UIPasteboard.general.string = selection.event.txHash
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        withAnimation { toastMessage = "Copied transaction hash" }
                    }
                )
                .presentationDetents([.medium, .large])
            }
            .fullScreenCover(isPresented: $showReceiveSheet) {
                ReceiveSheet(address: viewModel.address ?? "", publicKey: viewModel.publicKey ?? "", viewModel: viewModel)
            }
            .fullScreenCover(isPresented: $showSeedPhraseSheet) {
                SeedPhraseSheet(viewModel: viewModel)
            }
            .sheet(isPresented: $showContactList) {
                ContactListView(viewModel: viewModel)
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
                            announceToVoiceOver(message)
                            DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
                                withAnimation { toastMessage = nil }
                            }
                        }
                }
            }
            .animation(.easeInOut, value: toastMessage)
        }
    }

    // MARK: - Hamburger Menu

    private var hamburgerMenu: some View {
        HStack {
            Menu {
                Menu {
                    ForEach(WalletViewModel.supportedFiatCurrencies, id: \.code) { currency in
                        Button {
                            viewModel.setFiatCurrency(currency.code)
                        } label: {
                            HStack {
                                Text("\(WalletViewModel.fiatSymbol(for: currency.code)) \(currency.name)")
                                if viewModel.fiatCurrency == currency.code {
                                    Image(systemName: "checkmark")
                                }
                            }
                        }
                    }
                } label: {
                    Label("Fiat Currency (\(viewModel.fiatCurrency.uppercased()))", systemImage: "dollarsign.circle")
                }
                Button {
                    showContactList = true
                } label: {
                    Label("Contacts", systemImage: "person.crop.circle")
                }
                Button {
                    showSeedPhraseSheet = true
                } label: {
                    Label("Show Seed Phrase", systemImage: "key.viewfinder")
                }
} label: {
                Image(systemName: "line.3.horizontal")
                    .font(.title2)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
            }
            .accessibilityLabel("Menu")
            .accessibilityIdentifier("moreMenu")
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Balance Card

    private var balanceCard: some View {
        VStack(spacing: 8) {
                if viewModel.isBalanceHidden {
                    Text("\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}")
                        .font(.system(size: 40, weight: .bold, design: .monospaced))
                        .onTapGesture {
                            UIImpactFeedbackGenerator(style: .light).impactOccurred()
                            viewModel.isBalanceHidden.toggle()
                        }
                        .accessibilityLabel("Balance hidden")
                        .accessibilityHint("Double tap to show balance")
                        .accessibilityAddTraits(.isButton)
                } else if viewModel.showFiat {
                    if let fiat = viewModel.satsToFiat(viewModel.balanceSats ?? "0") {
                        Text(fiat)
                            .font(.system(size: 40, weight: .bold, design: .monospaced))
                            .contentTransition(.numericText())
                            .animation(.easeInOut(duration: 0.4), value: fiat)
                            .onTapGesture {
                                UIImpactFeedbackGenerator(style: .light).impactOccurred()
                                viewModel.isBalanceHidden.toggle()
                            }
                            .accessibilityLabel("Balance: \(fiat)")
                            .accessibilityHint("Double tap to hide balance")
                            .accessibilityAddTraits(.isButton)
                    } else if fiatPriceTimedOut {
                        Text(viewModel.balanceSats ?? "0")
                            .font(.system(size: 40, weight: .bold, design: .monospaced))
                            .onTapGesture {
                                UIImpactFeedbackGenerator(style: .light).impactOccurred()
                                viewModel.isBalanceHidden.toggle()
                            }
                        Text("Price unavailable")
                            .font(.caption)
                            .foregroundColor(Color.oubliOnSurfaceVariant)
                    } else {
                        ProgressView()
                            .frame(height: 48)
                            .accessibilityLabel("Loading price")
                            .onAppear {
                                viewModel.refreshBtcPrice()
                                fiatPriceTimedOut = false
                                DispatchQueue.main.asyncAfter(deadline: .now() + 8) {
                                    if viewModel.btcFiatPrice == nil {
                                        fiatPriceTimedOut = true
                                    }
                                }
                            }
                    }
                } else {
                    Text(viewModel.balanceSats ?? "0")
                        .font(.system(size: 40, weight: .bold, design: .monospaced))
                        .contentTransition(.numericText())
                        .animation(.easeInOut(duration: 0.4), value: viewModel.balanceSats)
                        .onTapGesture {
                            UIImpactFeedbackGenerator(style: .light).impactOccurred()
                            viewModel.isBalanceHidden.toggle()
                        }
                        .accessibilityLabel("Balance: \(viewModel.balanceSats ?? "0") sats")
                        .accessibilityHint("Double tap to hide balance")
                        .accessibilityAddTraits(.isButton)
                }

                Text(viewModel.showFiat ? viewModel.fiatCurrency.uppercased() : "sats")
                    .font(.title3)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .contentTransition(.interpolate)
                    .animation(.easeInOut(duration: 0.3), value: viewModel.showFiat)
                    .onTapGesture {
                        viewModel.showFiat.toggle()
                        if viewModel.showFiat && viewModel.btcFiatPrice == nil {
                            viewModel.refreshBtcPrice()
                        }
                    }
                    .accessibilityLabel("Currency: \(viewModel.showFiat ? viewModel.fiatCurrency.uppercased() : "sats")")
                    .accessibilityHint("Double tap to switch currency")
                    .accessibilityAddTraits(.isButton)

                if !viewModel.isBalanceHidden {
                    if let pending = viewModel.pendingSats, pending != "0" {
                        HStack(spacing: 4) {
                            Image(systemName: "clock")
                                .font(.caption)
                                .accessibilityHidden(true)
                            if viewModel.showFiat, let fiat = viewModel.satsToFiat(pending) {
                                Text("\(fiat) incoming")
                                    .font(.caption)
                            } else {
                                Text("\(pending) sats incoming")
                                    .font(.caption)
                            }
                        }
                        .foregroundColor(Color.oubliPending)
                        .modifier(PulseOpacity())
                        .padding(.top, 4)
                        .accessibilityElement(children: .combine)
                        .accessibilityLabel("Pending: \(viewModel.showFiat ? (viewModel.satsToFiat(pending) ?? pending) : pending) sats incoming")
                    }
                }
            }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
        .background(Color.oubliSurfaceContainerHigh)
        .cornerRadius(24)
        .padding(.horizontal, 24)
    }

    // MARK: - Action Buttons

    private var actionButtons: some View {
        HStack(spacing: 32) {
            actionButton(
                title: "Send",
                icon: "arrow.up",
                color: Color.oubliPrimary,
                identifier: "sendAction"
            ) {
                showSendSheet = true
            }
            actionButton(
                title: "Scan",
                icon: "camera.fill",
                color: Color.oubliOutline,
                identifier: "scanAction"
            ) {
                showQRScanner = true
            }
            actionButton(
                title: "Receive",
                icon: "arrow.down",
                color: Color.oubliPrimary,
                identifier: "receiveAction"
            ) {
                showReceiveSheet = true
            }
        }
        .padding(.horizontal, 24)
    }

    private func actionButton(
        title: String,
        icon: String,
        color: Color,
        identifier: String,
        action: @escaping () -> Void
    ) -> some View {
        Button {
            UIImpactFeedbackGenerator(style: .medium).impactOccurred()
            action()
        } label: {
            VStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.title3)
                    .foregroundStyle(Color.oubliOnPrimary)
                    .frame(width: 52, height: 52)
                    .background(color)
                    .clipShape(Circle())
                Text(title)
                    .font(.caption)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
            }
        }
        .accessibilityLabel(title)
        .accessibilityIdentifier(identifier)
    }

    // MARK: - Activity Section

    private var activitySection: some View {
        VStack(spacing: 12) {
            HStack {
                Text("Activity")
                    .font(.headline)
                    .accessibilityAddTraits(.isHeader)
                Spacer()
            }

            if viewModel.activity.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "clock")
                        .font(.system(size: 32))
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                        .accessibilityHidden(true)
                    Text("No transactions yet")
                        .font(.subheadline)
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                    if viewModel.balanceSats == "0" || viewModel.balanceSats == nil {
                        Text("Tap Receive to get your first payment")
                            .font(.caption)
                            .foregroundColor(Color.oubliPrimary)
                    }
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
                .background(Color.oubliSurfaceContainerHigh)
                .cornerRadius(12)
            } else {
                VStack(spacing: 6) {
                    ForEach(Array(viewModel.activity.enumerated()), id: \.offset) { index, event in
                        activityRow(event: event, index: index)
                            .background(Color.oubliSurfaceContainerHigh)
                            .cornerRadius(12)
                            .transition(.asymmetric(
                                insertion: .move(edge: .trailing).combined(with: .opacity),
                                removal: .opacity
                            ))
                    }
                }
                .animation(.easeInOut(duration: 0.3), value: viewModel.activity.count)
                .accessibilityIdentifier("activityList")
            }
        }
        .padding(.horizontal, 24)
    }

    private func contactNameForTx(_ txHash: String) -> String? {
        guard let recipient = viewModel.getTransferRecipient(txHash: txHash) else { return nil }
        return viewModel.findContactByAddress(recipient)?.name
    }

    private func activityRow(event: ActivityEventFfi, index: Int) -> some View {
        let contactName = contactNameForTx(event.txHash)
        return Button {
            selectedActivity = SelectedActivity(
                id: "\(event.txHash)-\(event.eventType)-\(index)",
                event: event
            )
        } label: {
            HStack(alignment: .top) {
                Image(systemName: activityIcon(event.eventType))
                    .foregroundStyle(activityColor(event.eventType))
                    .font(.body)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 6) {
                    Text(activityLabel(event.eventType))
                        .font(.subheadline.weight(.medium))
                    Text(contactName ?? shortHash(event.txHash))
                        .font(.caption.monospaced())
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                    HStack(spacing: 8) {
                        ActivityStatusBadge(status: activityStatus(event))
                        if let timestamp = formattedTimestamp(for: event) {
                            Text(timestamp)
                                .font(.caption2)
                                .foregroundColor(Color.oubliOnSurfaceVariant)
                        } else if event.blockNumber > 0 {
                            Text("Block \(event.blockNumber)")
                                .font(.caption2)
                                .foregroundColor(Color.oubliOnSurfaceVariant)
                        }
                    }
                }
                Spacer()
                if let amount = event.amountSats {
                    let sign = activityIsIncoming(event.eventType) ? "+" : "-"
                    Text("\(sign)\(amount) sats")
                        .font(.subheadline.monospaced().weight(.medium))
                        .foregroundColor(activityIsIncoming(event.eventType) ? Color.oubliReceived : Color.oubliSent)
                }
            }
            .padding(.vertical, 10)
            .padding(.horizontal, 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(activityLabel(event.eventType)): \(event.amountSats ?? "") sats, \(activityStatus(event))\(contactName != nil ? ", \(contactName!)" : ", transaction \(shortHash(event.txHash))")")
        .accessibilityIdentifier("activityRow_\(index)")
        .contextMenu {
            Button {
                UIPasteboard.general.string = event.txHash
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
                withAnimation { toastMessage = "Copied transaction hash" }
            } label: {
                Label("Copy Transaction Hash", systemImage: "doc.on.doc")
            }
            if let explorerURL = event.explorerUrl, let url = URL(string: explorerURL) {
                Link(destination: url) {
                    Label("View on Explorer", systemImage: "safari")
                }
            }
        }
    }

    private func activityStatus(_ event: ActivityEventFfi) -> String {
        let trimmed = event.status.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            return trimmed
        }
        return event.blockNumber == 0 ? "Pending" : "Confirmed"
    }

    private func formattedTimestamp(for event: ActivityEventFfi) -> String? {
        guard let timestamp = event.timestampSecs else {
            return activityStatus(event) == "Pending" ? "Awaiting confirmation" : nil
        }
        return ActivityDetailSheet.dateFormatter.string(
            from: Date(timeIntervalSince1970: TimeInterval(timestamp))
        )
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

    private func activityIcon(_ type: String) -> String {
        switch type {
        case "Fund": return "arrow.down.circle.fill"
        case "TransferOut": return "arrow.up.circle.fill"
        case "TransferIn": return "arrow.down.circle.fill"
        case "Withdraw": return "arrow.up.circle.fill"
        case "Rollover": return "checkmark.circle.fill"
        case "Ragequit": return "exclamationmark.triangle.fill"
        default: return "circle.fill"
        }
    }

    private func activityColor(_ type: String) -> Color {
        switch type {
        case "Fund", "TransferIn": return .oubliReceived
        case "TransferOut", "Withdraw": return .oubliSent
        case "Rollover": return .oubliPending
        case "Ragequit": return .oubliPending
        default: return .secondary
        }
    }

    private func shortHash(_ hash: String) -> String {
        if hash.count > 28 {
            return String(hash.prefix(16)) + "..." + String(hash.suffix(8))
        }
        return hash
    }

    // MARK: - Auto-fund Error Banner

    @ViewBuilder
    private var autoFundErrorBanner: some View {
        if let error = viewModel.autoFundError {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 4) {
                    Image(systemName: "exclamationmark.triangle")
                        .foregroundColor(.white)
                        .accessibilityHidden(true)
                    Text("Auto-fund error (tap to copy)")
                        .font(.caption.weight(.bold))
                }
                Text(error)
                    .font(.caption2)
            }
            .foregroundColor(.white)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(Color.oubliError.opacity(0.85))
            .cornerRadius(12)
            .padding(.horizontal, 24)
            .onTapGesture {
                UIPasteboard.general.string = error
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
                withAnimation { toastMessage = "Copied to clipboard" }
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Auto-fund error: \(error)")
            .accessibilityHint("Double tap to copy error to clipboard")
            .accessibilityAddTraits(.isButton)
        }
    }

    // MARK: - VoiceOver Announcement

    private func announceToVoiceOver(_ message: String) {
        UIAccessibility.post(notification: .announcement, argument: message)
    }
}

import SwiftUI

/// Onboarding flow: generate or restore a mnemonic.
struct OnboardingView: View {
    @EnvironmentObject var viewModel: WalletViewModel

    enum Step {
        case welcome
        case disclaimer
        case createOrRestore
        case showMnemonic
        case restoreMnemonic
    }

    @State private var step: Step = .welcome
    @State private var mnemonic: String = ""
    @State private var restorePhrase: String = ""
    @State private var mnemonicValidationError: String?
    @State private var disclaimerAccepted: Bool = false
    @State private var copiedToClipboard: Bool = false
    @State private var toastMessage: String? = nil
    @State private var showClipboardWarning: Bool = false

    var body: some View {
        NavigationStack {
            Group {
                switch step {
                case .welcome:
                    welcomeView
                case .disclaimer:
                    disclaimerView
                case .createOrRestore:
                    createOrRestoreView
                case .showMnemonic:
                    showMnemonicView
                case .restoreMnemonic:
                    restoreMnemonicView
                }
            }
            .navigationTitle("Oubli")
            .navigationBarTitleDisplayMode(.large)
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
                        UIAccessibility.post(notification: .announcement, argument: message)
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
                            withAnimation { toastMessage = nil }
                        }
                    }
            }
        }
        .animation(.easeInOut, value: toastMessage)
    }

    // MARK: - Welcome

    private var welcomeView: some View {
        VStack(spacing: 32) {
            Spacer()

            Image(systemName: "bitcoinsign.circle.fill")
                .font(.system(size: 80))
                .foregroundStyle(Color.oubliSecondary)
                .accessibilityHidden(true)

            Text("Welcome to Oubli")
                .font(.title)
                .fontWeight(.bold)
                .accessibilityAddTraits(.isHeader)

            Text("Your keys. Your Bitcoin. Secured by Starknet.")
                .font(.body)
                .foregroundColor(Color.oubliOnSurfaceVariant)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 40)

            Spacer()

            VStack(spacing: 12) {
                Button {
                    step = .disclaimer
                } label: {
                    Text("Get Started")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)

                Button {
                    step = .restoreMnemonic
                } label: {
                    Text("I already have a wallet")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderless)
                .foregroundColor(Color.oubliPrimary)
                .controlSize(.large)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
        }
    }

    // MARK: - Disclaimer

    private var disclaimerView: some View {
        VStack(spacing: 24) {
            Spacer()

            VStack(spacing: 20) {
                Image(systemName: "lock.shield.fill")
                    .font(.system(size: 60))
                    .foregroundStyle(Color.oubliSecondary)
                    .accessibilityHidden(true)

                Text("You Are in Control")
                    .font(.title2)
                    .fontWeight(.semibold)
                    .accessibilityAddTraits(.isHeader)

                Text("Oubli is a self-custodial wallet. You alone hold your private keys. No one \u{2014} not even Oubli \u{2014} can recover your funds if you lose your seed phrase. Make sure to back it up and store it safely.")
                    .font(.body)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .multilineTextAlignment(.center)

                Button {
                    disclaimerAccepted.toggle()
                } label: {
                    HStack(spacing: 12) {
                        Image(systemName: disclaimerAccepted ? "checkmark.square.fill" : "square")
                            .foregroundColor(disclaimerAccepted ? Color.oubliPrimary : Color.oubliOnSurfaceVariant)
                        Text("I understand that I am responsible for keeping my seed phrase safe.")
                            .font(.footnote)
                            .foregroundColor(Color.oubliOnSurface)
                            .multilineTextAlignment(.leading)
                    }
                }
                .accessibilityLabel("Disclaimer acknowledgment")
                .accessibilityValue(disclaimerAccepted ? "Accepted" : "Not accepted")
                .accessibilityHint("Double tap to toggle")
            }
            .padding(24)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
            .padding(.horizontal, 24)

            Spacer()

            Button {
                step = .createOrRestore
            } label: {
                Text("Continue")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
            .disabled(!disclaimerAccepted)
        }
    }

    // MARK: - Create or Restore

    private var createOrRestoreView: some View {
        VStack(spacing: 24) {
            Spacer()

            Text("Set Up Your Wallet")
                .font(.title2)
                .fontWeight(.semibold)
                .accessibilityAddTraits(.isHeader)

            Text("Create a new wallet or restore an existing one from your seed phrase.")
                .font(.body)
                .foregroundColor(Color.oubliOnSurfaceVariant)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Spacer()

            VStack(spacing: 16) {
                Button {
                    generateNewWallet()
                } label: {
                    Text("Create New Wallet")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)

                Button {
                    step = .restoreMnemonic
                } label: {
                    Text("Restore from Seed Phrase")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
        }
    }

    // MARK: - Show Mnemonic

    private var showMnemonicView: some View {
        ScrollView {
            VStack(spacing: 24) {
                Text("Your Seed Phrase")
                    .font(.title2)
                    .fontWeight(.semibold)
                    .accessibilityAddTraits(.isHeader)

                Text("Write down these words in order. You will need them to recover your wallet.")
                    .font(.body)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)

                mnemonicGrid

                Button {
                    showClipboardWarning = true
                } label: {
                    Label(copiedToClipboard ? "Copied!" : "Copy to Clipboard", systemImage: copiedToClipboard ? "checkmark" : "doc.on.doc")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
                .padding(.horizontal, 24)
                .accessibilityHint("Copies seed phrase to clipboard")
                .alert("Clipboard Warning", isPresented: $showClipboardWarning) {
                    Button("Copy Anyway", role: .destructive) {
                        UIPasteboard.general.string = mnemonic
                        copiedToClipboard = true
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        withAnimation { toastMessage = "Copied to clipboard" }
                        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                            copiedToClipboard = false
                        }
                    }
                    Button("Cancel", role: .cancel) {}
                } message: {
                    Text("Your seed phrase will be copied to the clipboard, where other apps may be able to read it. Only do this if you intend to paste it immediately and clear your clipboard afterward.")
                }

                warningBanner

                Button {
                    viewModel.completeOnboarding(mnemonic: mnemonic)
                } label: {
                    Text("I've Written It Down")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.horizontal, 24)
            }
            .padding(.vertical, 24)
        }
    }

    private var mnemonicGrid: some View {
        let words = mnemonic.split(separator: " ").map(String.init)

        return LazyVGrid(
            columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ],
            spacing: 12
        ) {
            ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                HStack(spacing: 4) {
                    Text(String(format: "%02d.", index + 1))
                        .font(.caption.monospaced())
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                        .frame(width: 28, alignment: .trailing)
                    Text(word)
                        .font(.body.monospaced())
                }
                .padding(.vertical, 8)
                .padding(.horizontal, 8)
                .background(Color.oubliSurfaceContainerHigh)
                .cornerRadius(8)
                .accessibilityElement(children: .combine)
                .accessibilityLabel("Word \(index + 1): \(word)")
            }
        }
        .padding(.horizontal, 24)
    }

    private var warningBanner: some View {
        HStack(spacing: 12) {
            Image(systemName: "exclamationmark.shield.fill")
                .foregroundColor(Color.oubliSecondary)
                .font(.title3)
                .accessibilityHidden(true)

            Text("Never share your seed phrase. Anyone who has it can steal your funds.")
                .font(.footnote)
                .foregroundColor(Color.oubliOnSurfaceVariant)
        }
        .padding()
        .background(Color.oubliSurfaceContainerHigh)
        .cornerRadius(12)
        .padding(.horizontal, 24)
    }

    // MARK: - Restore Mnemonic

    private var restoreMnemonicView: some View {
        ScrollView {
            VStack(spacing: 24) {
                Text("Restore Wallet")
                    .font(.title2)
                    .fontWeight(.semibold)
                    .accessibilityAddTraits(.isHeader)

                Text("Enter your 12-word seed phrase, separated by spaces.")
                    .font(.body)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)

                TextEditor(text: $restorePhrase)
                    .font(.body.monospaced())
                    .frame(minHeight: 120)
                    .padding(12)
                    .background(Color.oubliSurfaceContainerHigh)
                    .cornerRadius(12)
                    .padding(.horizontal, 24)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
                    .accessibilityLabel("Seed phrase input")

                if let error = mnemonicValidationError {
                    HStack(spacing: 4) {
                        Image(systemName: "exclamationmark.triangle")
                            .foregroundStyle(Color.oubliError)
                            .accessibilityHidden(true)
                        Text(error)
                            .font(.footnote)
                            .foregroundColor(Color.oubliError)
                    }
                }

                Button {
                    validateAndRestore()
                } label: {
                    Text("Restore Wallet")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.horizontal, 24)
                .disabled(restorePhrase.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(.vertical, 24)
        }
    }

    // MARK: - Actions

    private func generateNewWallet() {
        if let phrase = viewModel.generateMnemonic() {
            mnemonic = phrase
            step = .showMnemonic
        }
    }

    private func validateAndRestore() {
        let trimmed = restorePhrase.trimmingCharacters(in: .whitespacesAndNewlines)
        if viewModel.validateMnemonic(phrase: trimmed) {
            mnemonicValidationError = nil
            viewModel.completeOnboarding(mnemonic: trimmed)
        } else {
            mnemonicValidationError = "Invalid seed phrase. Please check your words and try again."
            UINotificationFeedbackGenerator().notificationOccurred(.error)
        }
    }
}

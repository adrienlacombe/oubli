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
                .foregroundStyle(.orange)

            Text("Welcome to Oubli")
                .font(.title)
                .fontWeight(.bold)

            Text("Your keys. Your Bitcoin. Secured by Starknet.")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 40)

            Spacer()

            Button {
                step = .disclaimer
            } label: {
                Text("Get Started")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
        }
    }

    // MARK: - Disclaimer

    private var disclaimerView: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "lock.shield.fill")
                .font(.system(size: 60))
                .foregroundStyle(.orange)

            Text("You Are in Control")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Oubli is a self-custodial wallet. You alone hold your private keys. No one \u{2014} not even Oubli \u{2014} can recover your funds if you lose your seed phrase. Make sure to back it up and store it safely.")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Button {
                disclaimerAccepted.toggle()
            } label: {
                HStack(spacing: 12) {
                    Image(systemName: disclaimerAccepted ? "checkmark.square.fill" : "square")
                        .foregroundColor(disclaimerAccepted ? .accentColor : .secondary)
                    Text("I understand that I am responsible for keeping my seed phrase safe.")
                        .font(.footnote)
                        .foregroundColor(.primary)
                        .multilineTextAlignment(.leading)
                }
            }
            .padding(.horizontal, 32)

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

            Text("Create a new wallet or restore an existing one from your seed phrase.")
                .font(.body)
                .foregroundColor(.secondary)
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

                Text("Write down these words in order. You will need them to recover your wallet.")
                    .font(.body)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)

                mnemonicGrid

                Button {
                    UIPasteboard.general.string = mnemonic
                    copiedToClipboard = true
                    withAnimation { toastMessage = "Copied to clipboard" }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                        copiedToClipboard = false
                    }
                } label: {
                    Label(copiedToClipboard ? "Copied!" : "Copy to Clipboard", systemImage: copiedToClipboard ? "checkmark" : "doc.on.doc")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
                .padding(.horizontal, 24)

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
                GridItem(.flexible()),
            ],
            spacing: 12
        ) {
            ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                HStack(spacing: 4) {
                    Text("\(index + 1).")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .frame(width: 24, alignment: .trailing)
                    Text(word)
                        .font(.body.monospaced())
                }
                .padding(.vertical, 8)
                .padding(.horizontal, 8)
                .background(Color(.systemGray6))
                .cornerRadius(8)
            }
        }
        .padding(.horizontal, 24)
    }

    private var warningBanner: some View {
        HStack(spacing: 12) {
            Image(systemName: "exclamationmark.shield.fill")
                .foregroundColor(.orange)
                .font(.title3)

            Text("Never share your seed phrase. Anyone who has it can steal your funds.")
                .font(.footnote)
                .foregroundColor(.secondary)
        }
        .padding()
        .background(Color(.systemGray6))
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

                Text("Enter your 12-word seed phrase, separated by spaces.")
                    .font(.body)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)

                TextEditor(text: $restorePhrase)
                    .font(.body.monospaced())
                    .frame(minHeight: 120)
                    .padding(12)
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    .padding(.horizontal, 24)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)

                if let error = mnemonicValidationError {
                    Text(error)
                        .font(.footnote)
                        .foregroundColor(.red)
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
        }
    }
}

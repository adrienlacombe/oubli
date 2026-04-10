import SwiftUI

// MARK: - Seed Phrase Sheet

struct SeedPhraseSheet: View {
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
                        announceToVoiceOver(message)
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
                        .foregroundStyle(Color.oubliError)
                        .accessibilityHidden(true)

                    Text("Write down these words in order and store them safely. Anyone with these words can access your funds.")
                        .font(.callout)
                        .foregroundColor(Color.oubliError)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)

                    VStack(alignment: .leading, spacing: 8) {
                        ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                            HStack {
                                Text("\(index + 1).")
                                    .font(.body.monospaced())
                                    .foregroundColor(Color.oubliOnSurfaceVariant)
                                    .frame(width: 30, alignment: .trailing)
                                Text(word)
                                    .font(.body.monospaced())
                            }
                            .accessibilityElement(children: .combine)
                            .accessibilityLabel("Word \(index + 1): \(word)")
                        }
                    }
                    .padding(20)
                    .background(Color.oubliSurfaceContainerHigh)
                    .cornerRadius(12)
                    .padding(.horizontal, 24)
                    .accessibilityIdentifier("seedWordsList")

                    Button {
                        UIPasteboard.general.string = words.joined(separator: " ")
                        copied = true
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
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
                    .accessibilityHint("Copies seed phrase to clipboard")
                }
                .padding(.vertical, 24)
            }
        } else {
            VStack(spacing: 24) {
                Spacer()

                if isLoading {
                    ProgressView("Loading...")
                        .accessibilityLabel("Loading seed phrase")
                } else {
                    Image(systemName: "key.viewfinder")
                        .font(.system(size: 48))
                        .foregroundStyle(Color.oubliOnSurfaceVariant)
                        .accessibilityHidden(true)

                    Text("Reveal your seed phrase")
                        .font(.headline)
                        .accessibilityAddTraits(.isHeader)

                    if let error = error {
                        HStack(spacing: 4) {
                            Image(systemName: "exclamationmark.triangle")
                                .foregroundStyle(Color.oubliError)
                                .accessibilityHidden(true)
                            Text(error)
                                .font(.callout)
                                .foregroundColor(Color.oubliError)
                        }
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
                UINotificationFeedbackGenerator().notificationOccurred(.error)
            }
        }
    }

    private func announceToVoiceOver(_ message: String) {
        UIAccessibility.post(notification: .announcement, argument: message)
    }
}

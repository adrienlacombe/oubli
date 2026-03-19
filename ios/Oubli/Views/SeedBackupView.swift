import SwiftUI

/// Seed backup flow: display word groups then verify selected words.
struct SeedBackupView: View {
    @EnvironmentObject var viewModel: WalletViewModel

    enum Phase {
        case loading
        case displayWords(SeedBackupStateFfi)
        case verify(SeedBackupStateFfi, currentPromptIndex: Int)
        case complete
        case failed
    }

    @State private var phase: Phase = .loading
    @State private var currentGroupIndex: Int = 0
    @State private var verifyAnswer: String = ""
    @State private var wrongAnswer: Bool = false

    var body: some View {
        NavigationStack {
            Group {
                switch phase {
                case .loading:
                    ProgressView("Preparing backup...")
                        .onAppear { loadBackup() }

                case .displayWords(let state):
                    wordGroupView(state: state)

                case .verify(let state, let promptIndex):
                    verificationView(state: state, promptIndex: promptIndex)

                case .complete:
                    completeView

                case .failed:
                    failedView
                }
            }
            .navigationTitle("Seed Backup")
            .navigationBarTitleDisplayMode(.inline)
        }
    }

    // MARK: - Word Group Display

    private func wordGroupView(state: SeedBackupStateFfi) -> some View {
        ScrollView {
            VStack(spacing: 24) {
                Text("Word Group \(currentGroupIndex + 1) of \(state.wordGroups.count)")
                    .font(.headline)

                Text("Write down these words carefully in order.")
                    .font(.body)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)

                let words = state.wordGroups[currentGroupIndex]
                let offset = wordsOffset(groupIndex: currentGroupIndex, groups: state.wordGroups)

                LazyVGrid(
                    columns: [GridItem(.flexible()), GridItem(.flexible())],
                    spacing: 12
                ) {
                    ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                        HStack(spacing: 4) {
                            Text("\(offset + index + 1).")
                                .font(.caption)
                                .foregroundColor(.secondary)
                                .frame(width: 28, alignment: .trailing)
                            Text(word)
                                .font(.body.monospaced())
                            Spacer()
                        }
                        .padding(.vertical, 10)
                        .padding(.horizontal, 12)
                        .background(Color(.systemGray6))
                        .cornerRadius(8)
                    }
                }
                .padding(.horizontal, 24)

                Button {
                    advanceGroup(state: state)
                } label: {
                    Text(currentGroupIndex < state.wordGroups.count - 1 ? "Next Group" : "Verify Words")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.horizontal, 24)
            }
            .padding(.vertical, 24)
        }
    }

    // MARK: - Verification

    private func verificationView(state: SeedBackupStateFfi, promptIndex: Int) -> some View {
        VStack(spacing: 24) {
            Spacer()

            let prompt = state.prompts[promptIndex]

            Text("Verify Word #\(prompt.wordNumber)")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Enter word number \(prompt.wordNumber) from your seed phrase.")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            TextField("Enter word", text: $verifyAnswer)
                .font(.body.monospaced())
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .padding()
                .background(Color(.systemGray6))
                .cornerRadius(12)
                .padding(.horizontal, 40)

            if wrongAnswer {
                Text("Incorrect. Please check your written words and try again.")
                    .font(.footnote)
                    .foregroundColor(.red)
                    .padding(.horizontal, 32)
            }

            Spacer()

            Text("Prompt \(promptIndex + 1) of \(state.prompts.count)")
                .font(.caption)
                .foregroundColor(.secondary)

            Button {
                checkAnswer(state: state, promptIndex: promptIndex)
            } label: {
                Text("Check")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
            .disabled(verifyAnswer.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    // MARK: - Complete

    private var completeView: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "checkmark.seal.fill")
                .font(.system(size: 64))
                .foregroundStyle(.green)

            Text("Backup Verified")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Your seed phrase has been verified successfully. Keep it stored safely offline.")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Spacer()

            Button {
                viewModel.dismissError() // refreshes state, returning to wallet
            } label: {
                Text("Done")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
        }
    }

    // MARK: - Failed

    private var failedView: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "xmark.circle.fill")
                .font(.system(size: 64))
                .foregroundStyle(.red)

            Text("Backup Failed")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Could not initialize the seed backup flow. Please try again.")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Spacer()

            Button {
                viewModel.dismissError()
            } label: {
                Text("Go Back")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
        }
    }

    // MARK: - Helpers

    private func loadBackup() {
        // In a real flow, the mnemonic would come from the ViewModel/core.
        // The seed backup is started by the wallet core; we just need the state.
        // For the shell, we attempt to start with an empty mnemonic which the core
        // should already have available internally.
        // A production app would pass the mnemonic through a secure channel.
        phase = .failed
    }

    /// Load backup with an explicit mnemonic (called from external navigation if available).
    func loadBackup(mnemonic: String) {
        if let state = viewModel.startSeedBackup(mnemonic: mnemonic) {
            currentGroupIndex = 0
            phase = .displayWords(state)
        } else {
            phase = .failed
        }
    }

    private func advanceGroup(state: SeedBackupStateFfi) {
        if currentGroupIndex < state.wordGroups.count - 1 {
            currentGroupIndex += 1
        } else {
            // Move to verification phase.
            if state.prompts.isEmpty {
                phase = .complete
            } else {
                verifyAnswer = ""
                wrongAnswer = false
                phase = .verify(state, currentPromptIndex: 0)
            }
        }
    }

    private func checkAnswer(state: SeedBackupStateFfi, promptIndex: Int) {
        let trimmed = verifyAnswer.trimmingCharacters(in: .whitespaces).lowercased()
        let correct = viewModel.verifySeedWord(promptIndex: UInt32(promptIndex), answer: trimmed)

        if correct {
            wrongAnswer = false
            verifyAnswer = ""
            let nextIndex = promptIndex + 1
            if nextIndex < state.prompts.count {
                phase = .verify(state, currentPromptIndex: nextIndex)
            } else {
                phase = .complete
            }
        } else {
            wrongAnswer = true
        }
    }

    private func wordsOffset(groupIndex: Int, groups: [[String]]) -> Int {
        var offset = 0
        for i in 0..<groupIndex {
            offset += groups[i].count
        }
        return offset
    }
}

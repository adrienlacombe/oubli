import SwiftUI

/// Root router that switches between screens based on the wallet state.
struct ContentView: View {
    @EnvironmentObject var viewModel: WalletViewModel
    @State private var toastMessage: String? = nil

    var body: some View {
        Group {
            if let initError = viewModel.initError {
                fatalErrorView(message: initError)
            } else {
                routedView
            }
        }
        .animation(.easeInOut(duration: 0.25), value: viewModel.currentState)
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
    private var routedView: some View {
        switch viewModel.currentState {
        case .onboarding:
            OnboardingView()

        case .locked:
            LockedView()

        case .ready:
            BalanceView()

        case .processing:
            ProcessingView()

        case .error:
            errorView

        case .seedBackup:
            SeedBackupView()

        case .wiped:
            wipedView
        }
    }

    // MARK: - Inline auxiliary views

    private var errorView: some View {
        VStack(spacing: 24) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 56))
                .foregroundColor(.red)

            Text("Something went wrong")
                .font(.title2)
                .fontWeight(.semibold)

            if let message = viewModel.errorMessage {
                Text(message)
                    .font(.body)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
                    .textSelection(.enabled)
            }

            HStack(spacing: 16) {
                if let message = viewModel.errorMessage {
                    Button {
                        UIPasteboard.general.string = message
                        withAnimation { toastMessage = "Copied to clipboard" }
                    } label: {
                        Label("Copy Error", systemImage: "doc.on.doc")
                    }
                    .buttonStyle(.bordered)
                }

                Button("Dismiss") {
                    viewModel.dismissError()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding()
    }

    private var wipedView: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "trash.fill")
                .font(.system(size: 56))
                .foregroundColor(.red)

            Text("Wallet Wiped")
                .font(.title2)
                .fontWeight(.semibold)

            Text("All data has been erased for your security.")
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Button("Set Up New Wallet") {
                viewModel.restartWallet()
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)

            Spacer()
        }
        .padding()
    }

    private func fatalErrorView(message: String) -> some View {
        VStack(spacing: 24) {
            Image(systemName: "xmark.octagon.fill")
                .font(.system(size: 56))
                .foregroundColor(.red)

            Text("Initialization Failed")
                .font(.title2)
                .fontWeight(.semibold)

            Text(message)
                .font(.body)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .padding()
    }
}

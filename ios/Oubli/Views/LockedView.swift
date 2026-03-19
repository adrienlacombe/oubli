import SwiftUI
import LocalAuthentication

/// Lock screen with biometric unlock.
struct LockedView: View {
    @EnvironmentObject var viewModel: WalletViewModel

    @State private var hasFiredAutoBiometric: Bool = false

    var body: some View {
        VStack(spacing: 32) {
            Spacer()

            Image(systemName: "lock.fill")
                .font(.system(size: 64))
                .foregroundStyle(.secondary)

            Text("Wallet Locked")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Authenticate to access your wallet.")
                .font(.body)
                .foregroundColor(.secondary)

            if let errorMessage = viewModel.biometricUnlockError {
                Text(errorMessage)
                    .font(.footnote)
                    .foregroundColor(.red)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
            }

            Spacer()

            Button {
                viewModel.unlockBiometric()
            } label: {
                Label(biometricButtonTitle, systemImage: biometricButtonIcon)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.horizontal, 24)
            .padding(.bottom, 40)
        }
        .onAppear {
            if !hasFiredAutoBiometric {
                hasFiredAutoBiometric = true
                viewModel.unlockBiometric()
            }
        }
    }

    private var biometricButtonTitle: String {
        let context = LAContext()
        var error: NSError?
        context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)

        switch context.biometryType {
        case .faceID:
            return "Unlock with Face ID"
        case .touchID:
            return "Unlock with Touch ID"
        default:
            return "Unlock with Biometrics"
        }
    }

    private var biometricButtonIcon: String {
        let context = LAContext()
        var error: NSError?
        context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error)

        switch context.biometryType {
        case .faceID:
            return "faceid"
        case .touchID:
            return "touchid"
        default:
            return "lock.open.fill"
        }
    }
}

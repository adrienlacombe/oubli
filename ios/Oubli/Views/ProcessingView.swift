import SwiftUI

/// Displayed while an async wallet operation (fund, transfer, withdraw, rollover) is in flight.
struct ProcessingView: View {
    @EnvironmentObject var viewModel: WalletViewModel

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            ProgressView()
                .scaleEffect(1.5)
                .padding(.bottom, 8)

            Text("Processing")
                .font(.title2)
                .fontWeight(.semibold)

            if let operation = viewModel.operation {
                Text(operationLabel(operation))
                    .font(.body)
                    .foregroundColor(.secondary)
            }

            if let address = viewModel.address {
                Text(address)
                    .font(.caption.monospaced())
                    .foregroundColor(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .padding(.horizontal, 40)
            }

            Spacer()

            Text("Please do not close the app.")
                .font(.footnote)
                .foregroundColor(.secondary)
                .padding(.bottom, 40)
        }
        .padding()
    }

    /// Convert the raw operation string from the core into a user-friendly label.
    private func operationLabel(_ operation: String) -> String {
        switch operation.lowercased() {
        case "fund":
            return "Processing..."
        case "transfer":
            return "Sending..."
        case "withdraw":
            return "Sending..."
        case "rollover":
            return "Settling incoming funds..."
        case "ragequit":
            return "Emergency withdrawal in progress..."
        case "refresh":
            return "Refreshing..."
        default:
            return operation
        }
    }
}

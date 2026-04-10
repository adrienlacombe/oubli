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
                .accessibilityLabel("Processing")

            Text("Processing")
                .font(.title2)
                .fontWeight(.semibold)
                .accessibilityAddTraits(.isHeader)

            if let operation = viewModel.operation {
                Text(operationLabel(operation))
                    .font(.body)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
            }

            if let address = viewModel.address {
                Text(address)
                    .font(.caption.monospaced())
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .padding(.horizontal, 40)
            }

            Spacer()

            Text("Please do not close the app.")
                .font(.footnote)
                .foregroundColor(Color.oubliOnSurfaceVariant)
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

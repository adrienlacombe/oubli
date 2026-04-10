import SwiftUI

/// Bidirectionally synced sats/fiat amount input.
/// Typing in either field updates the other in real time.
struct DualAmountInput: View {
    @Binding var satsAmount: String
    let satsToFiatRaw: (String) -> String?
    let fiatToSats: (String) -> String?
    let fiatCurrency: String
    let fiatSymbol: String
    var isReadOnly: Bool = false
    var showMaxButton: Bool = false
    var maxSats: String? = nil

    @State private var fiatAmount: String = ""
    /// Tracks which field the user is actively editing to prevent feedback loops.
    @State private var editingField: EditingField = .none

    private enum EditingField {
        case none, sats, fiat
    }

    private var hasFiatPrice: Bool {
        satsToFiatRaw("100000") != nil
    }

    var body: some View {
        VStack(spacing: 8) {
            // Sats field
            HStack(spacing: 6) {
                Text("sats")
                    .font(.caption.weight(.semibold))
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .frame(width: 36, alignment: .trailing)
                TextField("0", text: $satsAmount)
                    .keyboardType(.numberPad)
                    .font(.body.monospaced())
                    .textFieldStyle(.roundedBorder)
                    .disabled(isReadOnly)
                    .accessibilityLabel("Amount in sats")
                    .onChange(of: satsAmount) { newValue in
                        guard editingField != .fiat else { return }
                        editingField = .sats
                        if let converted = satsToFiatRaw(newValue), !newValue.isEmpty {
                            fiatAmount = converted
                        } else if newValue.isEmpty {
                            fiatAmount = ""
                        }
                        DispatchQueue.main.async { editingField = .none }
                    }
                if showMaxButton, let max = maxSats, !isReadOnly {
                    Button("Max") {
                        satsAmount = max
                    }
                    .font(.body.bold())
                    .accessibilityHint("Sets amount to full balance")
                }
            }

            // Fiat field
            HStack(spacing: 6) {
                Text(fiatSymbol)
                    .font(.caption.weight(.semibold))
                    .foregroundColor(Color.oubliOnSurfaceVariant)
                    .frame(width: 36, alignment: .trailing)
                if hasFiatPrice {
                    TextField("0.00", text: $fiatAmount)
                        .keyboardType(.decimalPad)
                        .font(.body.monospaced())
                        .textFieldStyle(.roundedBorder)
                        .disabled(isReadOnly)
                        .accessibilityLabel("Amount in \(fiatCurrency.uppercased())")
                        .onChange(of: fiatAmount) { newValue in
                            guard editingField != .sats else { return }
                            editingField = .fiat
                            if let converted = fiatToSats(newValue), !newValue.isEmpty {
                                satsAmount = converted
                            } else if newValue.isEmpty {
                                satsAmount = ""
                            }
                            DispatchQueue.main.async { editingField = .none }
                        }
                } else {
                    Text("Price unavailable")
                        .font(.caption)
                        .foregroundColor(Color.oubliOnSurfaceVariant)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 8)
                }
                Text(fiatCurrency.uppercased())
                    .font(.caption)
                    .foregroundColor(Color.oubliOnSurfaceVariant)
            }
        }
    }
}

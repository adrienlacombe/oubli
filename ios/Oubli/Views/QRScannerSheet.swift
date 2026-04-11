import SwiftUI

// MARK: - QR Scanner Sheet

struct QRScannerSheet: View {
    var onCodeScanned: (String) -> Void
    @Binding var isPresented: Bool

    var body: some View {
        NavigationStack {
            QRScannerView { code in
                isPresented = false
                onCodeScanned(code)
            }
            .ignoresSafeArea()
            .navigationTitle("Scan QR Code")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { isPresented = false }
                }
            }
        }
    }
}
